// Unit tests against the app's test vector (TestVector.mc, made by the
// Rust tests): the same keys, box, profile, holds and limits on both sides.
import Toybox.Cryptography;
import Toybox.Lang;
import Toybox.Test;

// The test vector's key pair and profile.
(:test)
module Vector {
    function pair() as Cryptography.KeyPair {
        return Seal.keyPairFrom(Seal.fromHex(TestVector.WATCH_SECRET));
    }

    function profile() as Profile {
        return Profile.parse(Seal.fromHex(TestVector.PROFILE)) as Profile;
    }
}

// The key pair from the reversed secret gives the wire key the app expects.
(:test)
function testWatchKey(logger as Test.Logger) as Boolean {
    var key = Seal.wireKey(Vector.pair());
    Test.assertMessage(key != null, "no public key");
    Test.assertMessage(Seal.same(key as ByteArray, Seal.fromBase64(TestVector.WATCH_KEY) as ByteArray),
        "public key: " + Seal.toBase64(key as ByteArray));
    return true;
}

// A key pair kept with the link comes back the same.
(:test)
function testKeptKeyPair(logger as Test.Logger) as Boolean {
    var pair = Seal.newKeyPair();
    var again = Seal.keyPairFromStored(Seal.privateBytes(pair) as ByteArray);
    Test.assertMessage(Seal.same(Seal.wireKey(pair) as ByteArray, Seal.wireKey(again) as ByteArray), "kept key");
    var plain = Seal.open(Seal.keyPairFromStored(Seal.privateBytes(Vector.pair()) as ByteArray),
        Seal.fromBase64(TestVector.APP_KEY) as ByteArray, Seal.fromBase64(TestVector.BOX) as ByteArray);
    Test.assertMessage(plain != null, "the kept key doesn't open the box");
    return true;
}

// Opening the box with that key and the app's key gives the profile.
(:test)
function testOpenBox(logger as Test.Logger) as Boolean {
    var plain = Seal.open(Vector.pair(), Seal.fromBase64(TestVector.APP_KEY) as ByteArray,
        Seal.fromBase64(TestVector.BOX) as ByteArray);
    Test.assertMessage(plain != null, "the box didn't open");
    Test.assertMessage(Seal.same(plain as ByteArray, Seal.fromHex(TestVector.PROFILE)), "wrong profile");
    return true;
}

// A changed byte anywhere, or a short box, is refused.
(:test)
function testChangedBoxIsRefused(logger as Test.Logger) as Boolean {
    var pair = Vector.pair();
    var app = Seal.fromBase64(TestVector.APP_KEY) as ByteArray;
    var box = Seal.fromBase64(TestVector.BOX) as ByteArray;
    var places = [0, 20, box.size() - 40, box.size() - 1];
    for (var i = 0; i < places.size(); i++) {
        var changed = box.slice(0, null);
        changed[places[i]] = changed[places[i]] ^ 1;
        Test.assertMessage(Seal.open(pair, app, changed) == null, "byte " + places[i]);
    }
    Test.assertMessage(Seal.open(pair, app, box.slice(0, box.size() - 16)) == null, "short box");
    Test.assertMessage(Seal.open(pair, box.slice(0, 64), box) == null, "wrong app key");
    return true;
}

(:test)
function testProfileHeader(logger as Test.Logger) as Boolean {
    var profile = Vector.profile();
    Test.assertMessage(profile != null, "the profile didn't parse");
    Test.assertEqualMessage(profile.name, TestVector.NAME, "name");
    Test.assertEqualMessage(profile.version, TestVector.VERSION, "version");
    Test.assertEqualMessage(profile.id, TestVector.ID, "id");
    Test.assertEqualMessage(profile.index, 0, "index");
    Test.assertEqualMessage(profile.count, 1, "count");
    return true;
}

// Each hold matches the app's within 0.001 mrad, weather past the grid too.
(:test)
function testHolds(logger as Test.Logger) as Boolean {
    var profile = Vector.profile();
    for (var i = 0; i < TestVector.HOLDS.size(); i++) {
        var row = TestVector.HOLDS[i];
        var hold = profile.hold(row[0], row[1], row[2], row[3], row[4]);
        Test.assertMessage(hold != null, "no hold at " + row[0]);
        var up = (hold as Hold).elevationMrad;
        var right = (hold as Hold).windageMrad;
        logger.debug(row[0] + " m: " + up + " up, " + right + " right");
        Test.assertMessage((up - row[5]).abs() < 0.001, "up at " + row[0] + ": " + up);
        Test.assertMessage((right - row[6]).abs() < 0.001, "right at " + row[0] + ": " + right);
    }
    return true;
}

// The ethical limit matches the app's within a metre.
(:test)
function testLimits(logger as Test.Logger) as Boolean {
    var profile = Vector.profile();
    for (var i = 0; i < TestVector.LIMITS.size(); i++) {
        var row = TestVector.LIMITS[i];
        var limit = profile.ethicalLimit(row[0], row[1]);
        Test.assertMessage(limit != null, "no limit");
        Test.assertMessage(((limit as Double) - row[2]).abs() < 0.5, "limit: " + limit);
    }
    return true;
}

// No answer before the first row or past the last.
(:test)
function testBeyondTheTable(logger as Test.Logger) as Boolean {
    var profile = Vector.profile();
    Test.assertMessage(profile.hold(profile.step - 1, 0, 3, 15, 1013) == null, "before the first row");
    Test.assertMessage(profile.hold(profile.lastM() + 1, 0, 3, 15, 1013) == null, "past the last row");
    Test.assertMessage(profile.hold(profile.lastM(), 0, 3, 15, 1013) != null, "the last row");
    Test.assertMessage(profile.hold(profile.step, 0, 3, 15, 1013) != null, "the first row");
    Test.assertMessage(profile.outsideGrid(-31, 1013), "too cold");
    Test.assertMessage(profile.outsideGrid(15, 1060), "too high a pressure");
    Test.assertMessage(!profile.outsideGrid(15, 1013), "a usual day");
    return true;
}

// Clicks and lengths at the target, and broken profiles refused.
(:test)
function testClicksAndRefusals(logger as Test.Logger) as Boolean {
    var profile = Vector.profile();
    // 0.1 mil clicks: 1.24 mrad is 12 clicks; 1 mrad at 300 m is 30 cm.
    Test.assertEqual(profile.clicks(1.24d), 12);
    Test.assertEqual(profile.clicks(-0.26d), -3);
    Test.assertMessage((profile.atTarget(1.0d, 300) - 30.0d).abs() < 1.0e-9, "cm");
    var bytes = Seal.fromHex(TestVector.PROFILE);
    Test.assertMessage(Profile.parse(bytes.slice(0, bytes.size() - 1)) == null, "short profile");
    var extended = bytes.slice(0, null);
    extended.add(0);
    Test.assertMessage(Profile.parse(extended) == null, "long profile");
    var wrongFormat = bytes.slice(0, null);
    wrongFormat[0] = 1;
    Test.assertMessage(Profile.parse(wrongFormat) == null, "format 1");
    // A click value under 0.001 is refused.
    var tinyClick = bytes.encodeNumber(0.0005, Lang.NUMBER_FORMAT_FLOAT, { :offset => 16, :endianness => Lang.ENDIAN_LITTLE });
    Test.assertMessage(Profile.parse(tinyClick) == null, "click value 0.0005");
    // The grid must rise.
    var unordered = bytes.slice(0, null);
    unordered[31] = unordered[30];
    Test.assertMessage(Profile.parse(unordered) == null, "temperatures not rising");
    // An index past the count.
    var misplaced = bytes.slice(0, null);
    misplaced[5] = 1;
    Test.assertMessage(Profile.parse(misplaced) == null, "index 1 of 1");
    return true;
}

// PKCS#7 padding is checked, not just cut.
(:test)
function testPadding(logger as Test.Logger) as Boolean {
    Test.assertMessage(Seal.unpad([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 3, 3, 3]b) != null, "good padding");
    Test.assertMessage(Seal.unpad([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 2, 3, 3]b) == null, "mixed padding");
    Test.assertMessage(Seal.unpad([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0]b) == null, "zero padding");
    Test.assertMessage(Seal.unpad([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 17]b) == null, "long padding");
    return true;
}
