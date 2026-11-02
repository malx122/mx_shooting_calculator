// The watch link's encryption (docs/watch.md, "Encryption"): ECDH on P-256
// between the watch's key pair and the phone's public key, then HMAC-SHA256
// and AES-256-CBC. It must agree byte for byte with
// crates/app/src/watch/seal.rs.
import Toybox.Cryptography;
import Toybox.Lang;
import Toybox.StringUtil;

module Seal {
    const IV_LEN = 16;
    const TAG_LEN = 32;
    const BLOCK = 16;
    // One coordinate of a P-256 point, bytes.
    const HALF = 32;
    // A public key on the wire: X then Y, each big-endian.
    const KEY_LEN = 64;
    // The largest box the server takes (decoded).
    const MAX_BOX = 12288;

    // A fresh key pair, for a new pairing.
    function newKeyPair() as Cryptography.KeyPair {
        return new Cryptography.KeyPair({
            :algorithm => Cryptography.KEY_PAIR_ELLIPTIC_CURVE_SECP256R1
        });
    }

    // The key pair for a private key given big-endian (the test vector's).
    // Connect IQ takes the number little-endian.
    function keyPairFrom(secret as ByteArray) as Cryptography.KeyPair {
        return new Cryptography.KeyPair({
            :algorithm => Cryptography.KEY_PAIR_ELLIPTIC_CURVE_SECP256R1,
            :privateKey => secret.reverse()
        });
    }

    // The key pair kept for a link (`privateBytes`), as Connect IQ gave it.
    function keyPairFromStored(secret as ByteArray) as Cryptography.KeyPair {
        return new Cryptography.KeyPair({
            :algorithm => Cryptography.KEY_PAIR_ELLIPTIC_CURVE_SECP256R1,
            :privateKey => secret
        });
    }

    // The private key's bytes, to keep with the link, or null if Connect
    // IQ gives none.
    function privateBytes(pair as Cryptography.KeyPair) as ByteArray or Null {
        return pair.getPrivateKey().getBytes();
    }

    // Reverses each 32-byte half of a 64-byte public key: Connect IQ's
    // little-endian X and Y to the wire's big-endian ones, and back.
    function swapHalves(key as ByteArray) as ByteArray {
        var out = key.slice(0, HALF).reverse();
        out.addAll(key.slice(HALF, KEY_LEN).reverse());
        return out;
    }

    // The key pair's public key as the wire has it, or null if Connect IQ
    // gives anything but X and Y.
    function wireKey(pair as Cryptography.KeyPair) as ByteArray or Null {
        var bytes = pair.getPublicKey().getBytes();
        if (bytes == null || bytes.size() != KEY_LEN) {
            return null;
        }
        return swapHalves(bytes);
    }

    // Checks and decrypts a box with the watch's key pair and the app's
    // public key (wire format). Null for anything that doesn't check out:
    // a wrong size, a wrong tag or wrong padding.
    function open(pair as Cryptography.KeyPair, appKey as ByteArray, box as ByteArray) as ByteArray or Null {
        var size = box.size();
        if (appKey.size() != KEY_LEN || size > MAX_BOX
                || size < IV_LEN + BLOCK + TAG_LEN
                || (size - IV_LEN - TAG_LEN) % BLOCK != 0) {
            return null;
        }
        var watchKey = wireKey(pair);
        if (watchKey == null) {
            return null;
        }
        var shared = sharedSecret(pair, appKey);
        if (shared == null) {
            return null;
        }
        var encKey = hmac(shared, [text("mxsc watch v1 enc")]);
        var macKey = hmac(shared, [text("mxsc watch v1 mac")]);
        var iv = box.slice(0, IV_LEN);
        var ciphertext = box.slice(IV_LEN, size - TAG_LEN);
        var tag = box.slice(size - TAG_LEN, size);
        var expected = hmac(macKey, [watchKey, appKey, iv, ciphertext]);
        if (!same(expected, tag)) {
            return null;
        }
        var plain;
        try {
            var cipher = new Cryptography.Cipher({
                :algorithm => Cryptography.CIPHER_AES256,
                :mode => Cryptography.MODE_CBC,
                :key => encKey,
                :iv => iv
            });
            plain = cipher.decrypt(ciphertext);
        } catch (e) {
            return null;
        }
        return unpad(plain);
    }

    // The ECDH shared secret, the X coordinate big-endian, or null.
    function sharedSecret(pair as Cryptography.KeyPair, appKey as ByteArray) as ByteArray or Null {
        try {
            var agreement = new Cryptography.KeyAgreement({
                :protocol => Cryptography.KEY_AGREEMENT_ECDH,
                :privateKey => pair.getPrivateKey()
            });
            // The declared parameter type is HashAlgorithm, but the value
            // wanted is the curve (docs: "a KEY_PAIR_ELLIPTIC_CURVE_* constant").
            agreement.addKey(Cryptography.createPublicKey(
                Cryptography.KEY_PAIR_ELLIPTIC_CURVE_SECP256R1 as Cryptography.HashAlgorithm,
                swapHalves(appKey)));
            var secret = agreement.generateSecret();
            // X alone, little-endian like Connect IQ's other numbers; if a
            // device ever gives X and Y, X is the first half.
            if (secret.size() == KEY_LEN) {
                secret = secret.slice(0, HALF);
            }
            if (secret.size() != HALF) {
                return null;
            }
            return secret.reverse();
        } catch (e) {
            return null;
        }
    }

    // HMAC-SHA256 over the parts in order.
    function hmac(key as ByteArray, parts as Array<ByteArray>) as ByteArray {
        var mac = new Cryptography.HashBasedMessageAuthenticationCode({
            :algorithm => Cryptography.HASH_SHA256,
            :key => key
        });
        for (var i = 0; i < parts.size(); i++) {
            mac.update(parts[i]);
        }
        return mac.digest();
    }

    // Strips PKCS#7 padding, or null if it is wrong.
    function unpad(plain as ByteArray) as ByteArray or Null {
        var size = plain.size();
        if (size == 0 || size % BLOCK != 0) {
            return null;
        }
        var pad = plain[size - 1];
        if (pad < 1 || pad > BLOCK) {
            return null;
        }
        for (var i = size - pad; i < size; i++) {
            if (plain[i] != pad) {
                return null;
            }
        }
        return plain.slice(0, size - pad);
    }

    // Equal bytes, compared all the way through.
    function same(a as ByteArray, b as ByteArray) as Boolean {
        if (a.size() != b.size()) {
            return false;
        }
        var difference = 0;
        for (var i = 0; i < a.size(); i++) {
            difference |= a[i] ^ b[i];
        }
        return difference == 0;
    }

    // The UTF-8 bytes of a string.
    function text(value as String) as ByteArray {
        return StringUtil.convertEncodedString(value, {
            :fromRepresentation => StringUtil.REPRESENTATION_STRING_PLAIN_TEXT,
            :toRepresentation => StringUtil.REPRESENTATION_BYTE_ARRAY,
            :encoding => StringUtil.CHAR_ENCODING_UTF8
        }) as ByteArray;
    }

    // Standard base64 to bytes, or null if it isn't base64.
    function fromBase64(value as Object or Null) as ByteArray or Null {
        if (!(value instanceof String) || value.length() == 0) {
            return null;
        }
        try {
            var bytes = StringUtil.convertEncodedString(value, {
                :fromRepresentation => StringUtil.REPRESENTATION_STRING_BASE64,
                :toRepresentation => StringUtil.REPRESENTATION_BYTE_ARRAY
            });
            return bytes instanceof ByteArray ? bytes : null;
        } catch (e) {
            return null;
        }
    }

    function toBase64(bytes as ByteArray) as String {
        return StringUtil.convertEncodedString(bytes, {
            :fromRepresentation => StringUtil.REPRESENTATION_BYTE_ARRAY,
            :toRepresentation => StringUtil.REPRESENTATION_STRING_BASE64
        }) as String;
    }

    function toHex(bytes as ByteArray) as String {
        return StringUtil.convertEncodedString(bytes, {
            :fromRepresentation => StringUtil.REPRESENTATION_BYTE_ARRAY,
            :toRepresentation => StringUtil.REPRESENTATION_STRING_HEX
        }) as String;
    }

    function fromHex(value as String) as ByteArray {
        return StringUtil.convertEncodedString(value, {
            :fromRepresentation => StringUtil.REPRESENTATION_STRING_HEX,
            :toRepresentation => StringUtil.REPRESENTATION_BYTE_ARRAY
        }) as ByteArray;
    }
}
