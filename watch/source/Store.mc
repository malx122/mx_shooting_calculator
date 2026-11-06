// What the watch keeps between runs: the link to the phone (its token, this
// watch's private key and the phone's public key), the profiles, the one
// chosen, and the last range, wind and weather set.
//
// The profiles are kept in one of two slots ("a0", "a1", ... or "b0", ...):
// new ones are written into the slot not in use, and only then does the
// bundle record point to it, so a half-written update never shows.
import Toybox.Application;
import Toybox.Application.Storage;
import Toybox.Cryptography;
import Toybox.Lang;

module Store {
    const LINK = "link";
    const BUNDLE = "bundle";
    const CHOSEN = "chosen";
    // In the unit named alongside, as last shown.
    const RANGE = "range";
    const RANGE_UNIT = "rangeUnit";
    const WIND = "wind";
    const WIND_UNIT = "windUnit";
    const CLOCK = "clock";
    // °C, and hPa when set by hand.
    const TEMPERATURE = "temperature";
    const PRESSURE = "pressure";

    // The chosen profile, read once and then kept in memory.
    var cached as Profile or Null = null;
    var loaded as Boolean = false;
    // A pairing's yes is on its way to the server, with its profiles in the
    // free slot: nothing else may write there until it is kept or given up.
    var saving as Boolean = false;

    // The link: { "token", "secret", "app" }, or null when not paired.
    function link() as Dictionary or Null {
        var value = Storage.getValue(LINK);
        if (value instanceof Dictionary && value["token"] instanceof String
                && value["secret"] instanceof ByteArray && value["app"] instanceof ByteArray) {
            return value as Dictionary;
        }
        return null;
    }

    function token() as String or Null {
        var kept = link();
        return kept != null ? kept["token"] as String : null;
    }

    // Keeps a new link; true if it was kept.
    function saveLink(token as String, pair as Cryptography.KeyPair, appKey as ByteArray) as Boolean {
        var secret = Seal.privateBytes(pair);
        if (secret == null) {
            return false;
        }
        try {
            Storage.setValue(LINK, {
                "token" => token,
                "secret" => secret,
                "app" => appKey
            } as Dictionary<Storage.KeyType, Storage.ValueType>);
        } catch (e) {
            return false;
        }
        return true;
    }

    // Puts back a link read before with `link()` (null: none).
    function restoreLink(old as Dictionary or Null) as Void {
        if (old == null) {
            forgetLink();
            return;
        }
        try {
            Storage.setValue(LINK, old as Dictionary<Storage.KeyType, Storage.ValueType>);
        } catch (e) {
            forgetLink();
        }
    }

    function forgetLink() as Void {
        Storage.deleteValue(LINK);
    }

    // { "version", "count", "slot", "names", "ids" }, or null.
    function bundle() as Dictionary or Null {
        var value = Storage.getValue(BUNDLE);
        if (value instanceof Dictionary && value["version"] instanceof Number
                && value["count"] instanceof Number && value["slot"] instanceof String
                && value["names"] instanceof Array && value["ids"] instanceof Array) {
            return value as Dictionary;
        }
        return null;
    }

    // The version kept, 0 for none.
    function version() as Number {
        var kept = bundle();
        return kept != null ? kept["version"] as Number : 0;
    }

    function names() as Array<String> {
        var kept = bundle();
        return kept != null ? kept["names"] as Array<String> : [] as Array<String>;
    }

    // Which profile is chosen: the one whose id was chosen last, else the
    // first.
    function chosenIndex() as Number {
        var kept = bundle();
        if (kept == null) {
            return 0;
        }
        var ids = kept["ids"] as Array<String>;
        var chosen = Storage.getValue(CHOSEN);
        for (var i = 0; i < ids.size(); i++) {
            if (ids[i].equals(chosen)) {
                return i;
            }
        }
        return 0;
    }

    function choose(index as Number) as Void {
        var kept = bundle();
        if (kept == null) {
            return;
        }
        var ids = kept["ids"] as Array<String>;
        if (index >= 0 && index < ids.size()) {
            save(CHOSEN, ids[index]);
            loaded = false;
        }
    }

    // The chosen profile, or null.
    function profile() as Profile or Null {
        if (!loaded) {
            cached = null;
            var kept = bundle();
            if (kept != null) {
                var index = chosenIndex();
                var bytes = Storage.getValue((kept["slot"] as String) + index);
                var read = bytes instanceof ByteArray ? Profile.parse(bytes) : null;
                // Only the bytes this bundle wrote there: never another
                // rifle's holds under this one's name.
                var ids = kept["ids"] as Array<String>;
                if (read != null && read.version == (kept["version"] as Number) && index < ids.size()
                        && read.id.equals(ids[index])) {
                    cached = read;
                }
            }
            loaded = true;
        }
        return cached;
    }

    // The slot an update is written into.
    function freeSlot() as String {
        var kept = bundle();
        return kept != null && (kept["slot"] as String).equals("a") ? "b" : "a";
    }

    // Keeps one profile in a slot not yet in use; true if it was kept.
    function saveItem(slot as String, index as Number, bytes as ByteArray) as Boolean {
        try {
            // Away first: replacing a value in place takes more memory.
            Storage.deleteValue(slot + index);
            Storage.setValue(slot + index, bytes);
        } catch (e) {
            return false;
        }
        return true;
    }

    // Makes the profiles written into `slot` the ones in use, and clears
    // the other slot; true if it worked.
    function commit(slot as String, version as Number, names as Array<String>, ids as Array<String>) as Boolean {
        var old = bundle();
        try {
            Storage.setValue(BUNDLE, {
                "version" => version,
                "count" => names.size(),
                "slot" => slot,
                "names" => names,
                "ids" => ids
            } as Dictionary<Storage.KeyType, Storage.ValueType>);
        } catch (e) {
            return false;
        }
        loaded = false;
        var other = slot.equals("a") ? "b" : "a";
        if (old != null && (old["slot"] as String).equals(other)) {
            clear(other);
        }
        return true;
    }

    // Lets go of profiles written for an update that wasn't kept (a no, or
    // a pairing given up), so they don't take up the watch's storage.
    function clearFree() as Void {
        clear(freeSlot());
    }

    function clear(slot as String) as Void {
        for (var i = 0; i < ProfileFormat.MAX_COUNT; i++) {
            Storage.deleteValue(slot + i);
        }
    }

    function number(key as String) as Numeric or Null {
        var value = Storage.getValue(key);
        return value instanceof Number || value instanceof Float
            || value instanceof Double || value instanceof Long ? value as Numeric : null;
    }

    function save(key as String, value as Number or Float or String) as Void {
        try {
            Storage.setValue(key, value);
        } catch (e) {
            // A full store only loses the last setting.
        }
    }
}
