// The watch's word to the server, sent on its own and tried again: how it
// went with what it fetched (POST /v1/watch/link/done {done, version}:
// used, declined, unreadable or left) and forgetting a link (DELETE
// /v1/watch/link). One request at a time, each tried again on a connection
// error, 429 or a server error, after 3, 10, 30, 60 and 120 s. It holds
// tokens and outcomes only, and works on after the screen that sent them
// has gone. Forgets are also kept in storage until the server has heard
// them, and sent again each time the app opens.
import Toybox.Application.Storage;
import Toybox.Communications;
import Toybox.Lang;
import Toybox.Timer;

module Outbox {
    // Waits before each retry, ms.
    const RETRY_MS = [3000, 10000, 30000, 60000, 120000];

    var sender as Sender or Null = null;

    function queue() as Sender {
        var current = sender;
        if (current == null) {
            current = new Sender();
            sender = current;
        }
        return current;
    }

    // How it went with version `version` (0 when there was none).
    function done(token as String, outcome as String, version as Number) as Void {
        queue().add([token, outcome, version, 0]);
    }

    // Forgets the link on the server.
    function forget(token as String) as Void {
        var kept = without(forgets(), token);
        kept.add(token);
        keepForgets(kept);
        queue().add([token, null, 0, 0]);
    }

    // Keeps a token to forget when the app next opens, without sending
    // anything now (a pairing that may not finish).
    function remember(token as String) as Void {
        var kept = without(forgets(), token);
        kept.add(token);
        keepForgets(kept);
    }

    // Takes a remembered token back (the pairing was kept).
    function unremember(token as String) as Void {
        keepForgets(without(forgets(), token));
    }

    // When the app opens: the forgets the server hasn't heard yet.
    function resume() as Void {
        var kept = forgets();
        for (var i = 0; i < kept.size(); i++) {
            queue().add([kept[i], null, 0, 0]);
        }
    }

    const FORGETS = "forgets";

    function forgets() as Array<String> {
        var value = Storage.getValue(FORGETS);
        return value instanceof Array ? value as Array<String> : [] as Array<String>;
    }

    // The tokens but `token` (compared as text).
    function without(tokens as Array<String>, token as String) as Array<String> {
        var out = [] as Array<String>;
        for (var i = 0; i < tokens.size(); i++) {
            if (!tokens[i].equals(token)) {
                out.add(tokens[i]);
            }
        }
        return out;
    }

    function keepForgets(tokens as Array<String>) as Void {
        try {
            Storage.setValue(FORGETS, tokens as Array<Storage.ValueType>);
        } catch (e) {
            // Then it is only tried while the app stays open.
        }
    }

    class Sender {
        // [token, outcome (null: forget), version, retries used], first in
        // first out.
        private var _queue as Array<Array> = [] as Array<Array>;
        private var _busy as Boolean = false;
        // The one retry timer, reused.
        private var _timer as Timer.Timer = new Timer.Timer();

        function initialize() {
        }

        function add(item as Array) as Void {
            _queue.add(item);
            if (!_busy) {
                next();
            }
        }

        // Sends the first in the queue.
        function next() as Void {
            if (_queue.size() == 0) {
                _busy = false;
                return;
            }
            _busy = true;
            var item = _queue[0];
            var headers = {
                "Authorization" => "Bearer " + (item[0] as String),
                "Content-Type" => Communications.REQUEST_CONTENT_TYPE_JSON
            };
            try {
                if (item[1] == null) {
                    Communications.makeWebRequest(Config.SERVER + "/v1/watch/link", null, {
                        :method => Communications.HTTP_REQUEST_METHOD_DELETE,
                        :headers => headers,
                        :responseType => Communications.HTTP_RESPONSE_CONTENT_TYPE_JSON
                    }, method(:onDone));
                } else {
                    var body = { "done" => item[1] as String } as Dictionary<Object, Object>;
                    if ((item[2] as Number) > 0) {
                        body["version"] = item[2] as Number;
                    }
                    Communications.makeWebRequest(Config.SERVER + "/v1/watch/link/done", body, {
                        :method => Communications.HTTP_REQUEST_METHOD_POST,
                        :headers => headers,
                        :responseType => Communications.HTTP_RESPONSE_CONTENT_TYPE_JSON
                    }, method(:onDone));
                }
            } catch (e) {
                onDone(Communications.UNKNOWN_ERROR, null);
            }
        }

        function onDone(responseCode as Number, data as Dictionary or String or Null) as Void {
            var item = _queue[0];
            var retries = item[3] as Number;
            if ((responseCode <= 0 || responseCode == 429 || responseCode >= 500) && retries < RETRY_MS.size()) {
                item[3] = retries + 1;
                _timer.start(method(:next), RETRY_MS[retries] as Number, false);
                return;
            }
            // Done, or given up on: a pairing expires on its own anyway, and
            // a forget is tried again when the app next opens.
            if (item[1] == null && responseCode > 0 && responseCode != 429 && responseCode < 500) {
                keepForgets(without(forgets(), item[0] as String));
            }
            _queue.remove(item);
            next();
        }
    }
}
