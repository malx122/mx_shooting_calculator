// Fetches the profiles the server holds for this watch (docs/watch.md,
// "Server API": GET /v1/watch/link, then each of /v1/watch/link/items/I),
// checks and opens each with the watch's key pair and the phone's public
// key, and writes them into the slot not in use. Making them the ones in
// use (Store.commit) is up to the caller. One request at a time.
import Toybox.Communications;
import Toybox.Cryptography;
import Toybox.Lang;

class Fetcher {
    enum Result {
        // Pairing: the phone hasn't sent anything yet.
        RESULT_WAITING,
        // Nothing newer than what the watch has.
        RESULT_SAME,
        // All fetched and written: version, names, ids and slot are set.
        RESULT_FETCHED,
        // The server doesn't know this watch (any more).
        RESULT_GONE,
        // Something didn't open, check out or parse.
        RESULT_UNREADABLE,
        // No answer, or not a usable one: try again later (responseCode).
        RESULT_FAILED
    }

    var version as Number = 0;
    var count as Number = 0;
    var names as Array<String> = [] as Array<String>;
    var ids as Array<String> = [] as Array<String>;
    var slot as String = "a";
    // The phone's public key: the one kept, or (pairing) the one sent.
    var appKey as ByteArray or Null;
    var responseCode as Number = 0;

    private var _token as String;
    private var _pair as Cryptography.KeyPair;
    // Only fetch a version newer than this.
    private var _have as Number;
    private var _done as Method(result as Number) as Void;
    private var _index as Number = 0;
    private var _stopped as Boolean = false;

    // `appKey` null while pairing: the watch takes the key the server gives,
    // and the hunter checks the names before it is kept.
    function initialize(token as String, pair as Cryptography.KeyPair, appKey as ByteArray or Null,
            have as Number, done as Method(result as Number) as Void) {
        _token = token;
        _pair = pair;
        self.appKey = appKey;
        _have = have;
        _done = done;
    }

    function start() as Void {
        request("/v1/watch/link", method(:onSummary));
    }

    // Whatever arrives later is dropped.
    function stop() as Void {
        _stopped = true;
    }

    private function request(path as String,
            callback as Method(responseCode as Number, data as Dictionary or String or Null) as Void) as Void {
        try {
            Communications.makeWebRequest(Config.SERVER + path, null, {
                :method => Communications.HTTP_REQUEST_METHOD_GET,
                :headers => { "Authorization" => "Bearer " + _token },
                :responseType => Communications.HTTP_RESPONSE_CONTENT_TYPE_JSON
            }, callback);
        } catch (e) {
            finish(RESULT_FAILED, Communications.BLE_CONNECTION_UNAVAILABLE);
        }
    }

    private function finish(result as Number, code as Number) as Void {
        if (!_stopped) {
            _stopped = true;
            responseCode = code;
            _done.invoke(result);
        }
    }

    function onSummary(code as Number, data as Dictionary or String or Null) as Void {
        if (_stopped) {
            return;
        }
        if (code == 404) {
            finish(RESULT_GONE, code);
            return;
        }
        if (code != 200 || !(data instanceof Dictionary)) {
            finish(RESULT_FAILED, code);
            return;
        }
        if (data["waiting"] == true) {
            finish(RESULT_WAITING, code);
            return;
        }
        var key = Seal.fromBase64(data["key"] as Object or Null);
        var newVersion = data["version"];
        var newCount = data["count"];
        if (key == null || !(newVersion instanceof Number) || !(newCount instanceof Number)
                || newCount < 1 || newCount > ProfileFormat.MAX_COUNT) {
            finish(RESULT_UNREADABLE, code);
            return;
        }
        var kept = appKey;
        if (kept != null && !Seal.same(kept, key)) {
            // Not the phone the watch is paired with.
            finish(RESULT_UNREADABLE, code);
            return;
        }
        if (newVersion <= _have) {
            finish(RESULT_SAME, code);
            return;
        }
        appKey = key;
        version = newVersion;
        count = newCount;
        slot = Store.freeSlot();
        _index = 0;
        request("/v1/watch/link/items/0", method(:onItem));
    }

    function onItem(code as Number, data as Dictionary or String or Null) as Void {
        if (_stopped) {
            return;
        }
        if (code != 200 || !(data instanceof Dictionary)) {
            // 404: replaced while fetching; it is fetched again next time.
            finish(RESULT_FAILED, code);
            return;
        }
        if (data["version"] != version) {
            finish(RESULT_FAILED, code);
            return;
        }
        var box = Seal.fromBase64(data["box"] as Object or Null);
        var key = appKey;
        var plain = box != null && key != null ? Seal.open(_pair, key, box) : null;
        var profile = plain != null ? Profile.parse(plain) : null;
        // The bundle's version, place and size are inside the box too, so
        // nobody can mix in a box from another bundle or another place.
        if (profile == null || profile.version != version || profile.index != _index
                || profile.count != count) {
            finish(RESULT_UNREADABLE, code);
            return;
        }
        if (!Store.saveItem(slot, _index, plain as ByteArray)) {
            finish(RESULT_UNREADABLE, code);
            return;
        }
        names.add(profile.name);
        ids.add(profile.id);
        _index += 1;
        if (_index < count) {
            request("/v1/watch/link/items/" + _index, method(:onItem));
        } else {
            finish(RESULT_FETCHED, code);
        }
    }
}
