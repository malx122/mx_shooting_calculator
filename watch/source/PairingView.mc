// Pair with phone (docs/watch.md, "Pairing"): a fresh key pair, a code to
// type into the app, a wait for the encrypted profiles, then "Use these
// rifles?" and a word to the server on how it went. On yes the watch keeps
// the link, and later updates come without asking (Updater).
import Toybox.Communications;
import Toybox.Cryptography;
import Toybox.Graphics;
import Toybox.Lang;
import Toybox.System;
import Toybox.Timer;
import Toybox.WatchUi;

class PairingView extends WatchUi.View {
    enum State {
        STATE_ASKING,
        STATE_WAITING,
        // Asking the hunter whether to use them.
        STATE_CONFIRMING,
        // Yes: telling the server, which must take the pairing before the
        // watch keeps it.
        STATE_SAVING,
        STATE_RECEIVED,
        // Answered no: back to the main screen.
        STATE_DECLINED,
        STATE_FAILED
    }
    // How often to ask for the delivery, ms.
    const POLL_MS = 3000;
    // A pairing lives ten minutes at most.
    const MAX_WAIT_MS = 600000;
    // How long "Received" shows before the main screen, ms.
    const RECEIVED_MS = 2500;
    // The shortest a timer can be, ms.
    const SOON_MS = 50;
    // How long to wait for the answer once the question has gone, ms.
    const ANSWER_GRACE_MS = 2000;
    // Telling the server yes: tries, and the wait between them, ms.
    const SAVE_TRIES = 3;
    const SAVE_RETRY_MS = 2000;

    private var _state as Number = STATE_ASKING;
    private var _saveTries as Number = 0;
    // Kept with the link on yes; otherwise only in memory.
    private var _pair as Cryptography.KeyPair or Null;
    private var _code as String or Null;
    private var _token as String or Null;
    // The view's one timer, started and stopped as needed.
    private var _timer as Timer.Timer = new Timer.Timer();
    private var _ticking as Boolean = false;
    private var _inFlight as Boolean = false;
    private var _deadline as Number = 0;
    // What went wrong, or a passing error while waiting.
    private var _message as String or Null;
    // The profiles waiting for a yes, written but not yet in use.
    private var _pending as Fetcher or Null;
    private var _name as String = "";
    private var _shown as Boolean = false;
    private var _started as Boolean = false;
    // Left with BACK: whatever still arrives is ignored.
    private var _closed as Boolean = false;
    // The outcome for this token has gone to the server (one at most).
    private var _reported as Boolean = false;

    function initialize() {
        View.initialize();
    }

    function onShow() as Void {
        _shown = true;
        if (_closed) {
            return;
        }
        if (!_started) {
            start();
        } else if (_state == STATE_WAITING) {
            startTimer(POLL_MS, true);
        } else if (_state == STATE_CONFIRMING) {
            // Back from the question: the answer comes in a moment; if it
            // never does, the question went away unanswered, as good as no.
            startTimer(ANSWER_GRACE_MS, false);
        } else if (_state == STATE_SAVING && !_ticking) {
            // Covered while telling the server: try again.
            startTimer(SAVE_RETRY_MS, false);
        } else if ((_state == STATE_RECEIVED || _state == STATE_DECLINED) && !_ticking) {
            startTimer(_state == STATE_RECEIVED ? RECEIVED_MS : SOON_MS, false);
        }
    }

    // Leaving the view, or covering it with the question, stops the polling.
    function onHide() as Void {
        _shown = false;
        stopTimer();
    }

    // BACK, or done: nothing that arrives later is used. A watch that leaves
    // before it has the rifles says so.
    function close() as Void {
        // A question that went away without a yes is a no.
        report(_state == STATE_CONFIRMING ? "declined" : "left", 0);
        _closed = true;
        // A yes on its way to the server finishes on its own, tries and all.
        if (_state != STATE_SAVING) {
            stopTimer();
            _pair = null;
            stopFetcher();
            if (_started) {
                Store.clearFree();
            }
        }
    }

    private function stopFetcher() as Void {
        var fetcher = _pending;
        if (fetcher != null) {
            fetcher.stop();
        }
        _pending = null;
        _inFlight = false;
    }

    function canRetry() as Boolean {
        return _state == STATE_FAILED && !_inFlight && !_closed;
    }

    // Starts a new pairing, with a new key pair.
    function start() as Void {
        _started = true;
        // An update fetching meanwhile would write into the same slot.
        Updater.stop();
        stopTimer();
        stopFetcher();
        _code = null;
        _token = null;
        _reported = false;
        _message = null;
        _state = STATE_ASKING;
        _pair = Seal.newKeyPair();
        var key = Seal.wireKey(_pair);
        if (key == null) {
            fail(Lang.format(Ui.str(Rez.Strings.ConnectionError), ["key"]));
            return;
        }
        request(Config.SERVER + "/v1/watch/pairings", { "key" => Seal.toBase64(key) } as Dictionary<Object, Object>,
            Communications.HTTP_REQUEST_METHOD_POST, method(:onPaired));
        WatchUi.requestUpdate();
    }

    private function request(url as String, params as Dictionary<Object, Object> or Null,
            httpMethod as Communications.HttpRequestMethod,
            callback as Method(responseCode as Number, data as Dictionary or String or Null) as Void) as Void {
        _inFlight = true;
        try {
            Communications.makeWebRequest(url, params, {
                :method => httpMethod,
                :headers => { "Content-Type" => Communications.REQUEST_CONTENT_TYPE_JSON },
                :responseType => Communications.HTTP_RESPONSE_CONTENT_TYPE_JSON
            }, callback);
        } catch (e) {
            _inFlight = false;
            fail(Ui.str(Rez.Strings.NoPhone));
        }
    }

    function onPaired(responseCode as Number, data as Dictionary or String or Null) as Void {
        _inFlight = false;
        if (_closed || _state != STATE_ASKING) {
            return;
        }
        if (responseCode != 200) {
            fail(problem(responseCode));
            return;
        }
        var code = data instanceof Dictionary ? data["code"] : null;
        var token = data instanceof Dictionary ? data["token"] : null;
        var expiresIn = data instanceof Dictionary ? data["expires_in"] : null;
        if (!(code instanceof String) || code.length() != 8
                || !(token instanceof String) || token.length() != 64) {
            fail(Lang.format(Ui.str(Rez.Strings.ServerError), [responseCode]));
            return;
        }
        _code = code;
        _token = token;
        var wait = MAX_WAIT_MS;
        if (expiresIn instanceof Number && expiresIn > 0 && expiresIn * 1000 < wait) {
            wait = expiresIn * 1000;
        }
        _deadline = System.getTimer() + wait;
        _state = STATE_WAITING;
        if (_shown) {
            startTimer(POLL_MS, true);
        }
        WatchUi.requestUpdate();
    }

    // Asks once whether the phone has sent the rifles, and fetches them
    // when it has; never more than one request at a time.
    function poll() as Void {
        if (_closed || _state != STATE_WAITING || _inFlight || !_shown) {
            return;
        }
        var pair = _pair;
        var token = _token;
        if (pair == null || token == null) {
            return;
        }
        _inFlight = true;
        var fetcher = new Fetcher(token, pair, null, 0, method(:onFetched));
        _pending = fetcher;
        fetcher.start();
    }

    function onFetched(result as Number) as Void {
        _inFlight = false;
        var fetcher = _pending;
        if (_closed || fetcher == null || _state != STATE_WAITING) {
            return;
        }
        if (result == Fetcher.RESULT_FETCHED) {
            confirm(fetcher);
            return;
        }
        _pending = null;
        if (result == Fetcher.RESULT_WAITING) {
            // The code's own ten minutes; once the phone has sent, the
            // server gives more, and only it says when they are up.
            if (System.getTimer() - _deadline >= 0) {
                fail(Ui.str(Rez.Strings.Expired));
                return;
            }
            setMessage(null);
        } else if (result == Fetcher.RESULT_GONE) {
            fail(Ui.str(Rez.Strings.Expired));
        } else if (result == Fetcher.RESULT_UNREADABLE) {
            report("unreadable", fetcher.version);
            fail(Ui.str(Rez.Strings.BadBox));
        } else {
            // It may pass (the phone out of reach for a moment, a busy
            // server, a box replaced while fetching): say so and keep
            // asking until the code expires.
            setMessage(problem(fetcher.responseCode));
        }
    }

    // Asks whether to use the rifles. Polling stops: they stay on the
    // server until the watch answers, so nothing is lost.
    private function confirm(fetcher as Fetcher) as Void {
        stopTimer();
        _name = list(fetcher.names);
        _state = STATE_CONFIRMING;
        _message = null;
        WatchUi.pushView(new WatchUi.Confirmation(Ui.str(Rez.Strings.UseRifles) + "\n" + _name),
            new UseRiflesDelegate(self), WatchUi.SLIDE_UP);
    }

    // The first names, one a line, and how many more.
    private function list(names as Array<String>) as String {
        var shown = names.size() > 3 ? 2 : names.size();
        var text = "";
        for (var i = 0; i < shown; i++) {
            text += (i > 0 ? "\n" : "") + names[i];
        }
        if (names.size() > shown) {
            text += "\n" + Lang.format(Ui.str(Rez.Strings.AndMore), [names.size() - shown]);
        }
        return text;
    }

    // The hunter's answer to "Use these rifles?". On yes the server is told
    // first, and the watch keeps the rifles and the link only once the
    // server has taken the pairing (it may have run out meanwhile).
    function answer(yes as Boolean) as Void {
        if (_closed || _state != STATE_CONFIRMING) {
            return;
        }
        var fetcher = _pending;
        var token = _token;
        if (yes && fetcher != null && _pair != null && token != null) {
            _reported = true;
            _state = STATE_SAVING;
            Store.saving = true;
            _saveTries = 0;
            // Should the yes not finish (BACK, the app closing), the pairing
            // is let go on the server when the app next opens; keeping it
            // takes this back.
            Outbox.remember(token);
            sendYes();
        } else {
            // What the watch had stays.
            _pending = null;
            _pair = null;
            Store.clearFree();
            report("declined", fetcher != null ? fetcher.version : 0);
            _state = STATE_DECLINED;
            startTimer(SOON_MS, false);
        }
        WatchUi.requestUpdate();
    }

    // POST .../done {used}: the answer says whether the server took it.
    function sendYes() as Void {
        var fetcher = _pending;
        var token = _token;
        if (fetcher == null || token == null) {
            return;
        }
        _saveTries += 1;
        try {
            Communications.makeWebRequest(Config.SERVER + "/v1/watch/link/done",
                { "done" => "used", "version" => fetcher.version } as Dictionary<Object, Object>, {
                    :method => Communications.HTTP_REQUEST_METHOD_POST,
                    :headers => {
                        "Authorization" => "Bearer " + token,
                        "Content-Type" => Communications.REQUEST_CONTENT_TYPE_JSON
                    },
                    :responseType => Communications.HTTP_RESPONSE_CONTENT_TYPE_JSON
                }, method(:onSaved));
        } catch (e) {
            onSaved(Communications.BLE_CONNECTION_UNAVAILABLE, null);
        }
    }

    // The server's answer to yes. Even after BACK the rifles are kept if
    // the server took the pairing, so both sides agree.
    function onSaved(responseCode as Number, data as Dictionary or String or Null) as Void {
        if (_state != STATE_SAVING) {
            return;
        }
        var fetcher = _pending;
        var token = _token;
        var pair = _pair;
        if (fetcher == null || token == null || pair == null) {
            return;
        }
        if (responseCode == 200 && data instanceof Dictionary && data["taken"] == true) {
            keep(fetcher, token, pair);
            return;
        }
        if (responseCode == 200) {
            // The server no longer had the pairing: it ran out.
            finishFailed(Ui.str(Rez.Strings.Expired));
            return;
        }
        if (_saveTries < SAVE_TRIES) {
            startTimer(SAVE_RETRY_MS, false);
            return;
        }
        // The server may have taken it without the answer getting back:
        // let it go there too, so neither side keeps half a pairing.
        Outbox.forget(token);
        finishFailed(problem(responseCode));
    }

    // Keeps the link, then the rifles; on any failure the old link is put
    // back as it was and the new one is let go on the server.
    private function keep(fetcher as Fetcher, token as String, pair as Cryptography.KeyPair) as Void {
        Store.saving = false;
        var appKey = fetcher.appKey;
        var oldLink = Store.link();
        var old = Store.token();
        if (appKey == null || !Store.saveLink(token, pair, appKey)) {
            Outbox.forget(token);
            finishFailed(Ui.str(Rez.Strings.NotKept));
            return;
        }
        if (!Store.commit(fetcher.slot, fetcher.version, fetcher.names, fetcher.ids)) {
            Store.restoreLink(oldLink);
            Outbox.forget(token);
            finishFailed(Ui.str(Rez.Strings.NotKept));
            return;
        }
        Outbox.unremember(token);
        // The phone paired before is let go.
        if (old != null && !old.equals(token)) {
            Outbox.forget(old);
        }
        _pending = null;
        _pair = null;
        _state = STATE_RECEIVED;
        if (!_closed) {
            startTimer(RECEIVED_MS, false);
            WatchUi.requestUpdate();
        }
    }

    private function finishFailed(message as String) as Void {
        Store.saving = false;
        Store.clearFree();
        _pending = null;
        if (_closed) {
            _state = STATE_FAILED;
            _pair = null;
            return;
        }
        fail(message);
    }

    // Tells the server how it went (used, declined, unreadable, left). Once
    // per token; it goes out on its own and doesn't hold up the screen.
    private function report(outcome as String, version as Number) as Void {
        var token = _token;
        if (token != null && !_reported) {
            _reported = true;
            Outbox.done(token, outcome, version);
        }
    }

    function onTimer() as Void {
        if (_state != STATE_WAITING) {
            // Only the polling timer repeats.
            _ticking = false;
        }
        if (_closed && _state != STATE_SAVING) {
            stopTimer();
            return;
        }
        if (_state == STATE_WAITING) {
            poll();
        } else if (_state == STATE_CONFIRMING) {
            stopTimer();
            answer(false);
        } else if (_state == STATE_SAVING) {
            sendYes();
        } else if (_state == STATE_RECEIVED || _state == STATE_DECLINED) {
            stopTimer();
            if (_shown) {
                close();
                WatchUi.popView(WatchUi.SLIDE_RIGHT);
            }
        }
    }

    // Stops waiting, for good: a pairing that is given up says so.
    private function fail(message as String) as Void {
        report("left", 0);
        stopTimer();
        _pair = null;
        stopFetcher();
        Store.clearFree();
        _state = STATE_FAILED;
        _message = message;
        WatchUi.requestUpdate();
    }

    private function setMessage(message as String or Null) as Void {
        var changed = message == null ? _message != null : !message.equals(_message);
        _message = message;
        if (changed) {
            WatchUi.requestUpdate();
        }
    }

    // What a failed request means, for the hunter.
    private function problem(responseCode as Number) as String {
        if (responseCode == Communications.BLE_CONNECTION_UNAVAILABLE
                || responseCode == Communications.BLE_HOST_TIMEOUT
                || responseCode == Communications.BLE_ERROR) {
            return Ui.str(Rez.Strings.NoPhone);
        }
        if (responseCode == 404) {
            return Ui.str(Rez.Strings.Expired);
        }
        if (responseCode == 429) {
            return Ui.str(Rez.Strings.Busy);
        }
        if (responseCode < 0) {
            return Lang.format(Ui.str(Rez.Strings.ConnectionError), [responseCode]);
        }
        return Lang.format(Ui.str(Rez.Strings.ServerError), [responseCode]);
    }

    private function startTimer(ms as Number, repeat as Boolean) as Void {
        stopTimer();
        _timer.start(method(:onTimer), ms, repeat);
        _ticking = true;
    }

    private function stopTimer() as Void {
        if (_ticking) {
            _timer.stop();
            _ticking = false;
        }
    }

    function onUpdate(dc as Graphics.Dc) as Void {
        dc.setColor(Ui.TEXT, Ui.BACKGROUND);
        dc.clear();
        var w = dc.getWidth();
        var h = dc.getHeight();
        var cx = w / 2;
        var center = Graphics.TEXT_JUSTIFY_CENTER | Graphics.TEXT_JUSTIFY_VCENTER;
        if (_state == STATE_ASKING || _state == STATE_SAVING) {
            dc.drawText(cx, h / 2, Graphics.FONT_SMALL,
                Ui.str(_state == STATE_SAVING ? Rez.Strings.Saving : Rez.Strings.AskingForCode), center);
            return;
        }
        if (_state == STATE_RECEIVED) {
            dc.drawText(cx, h / 2, Graphics.FONT_SMALL,
                Ui.wrap(Lang.format(Ui.str(Rez.Strings.Received), [_name]), Graphics.FONT_SMALL, w * 0.8, h * 0.5),
                center);
            return;
        }
        if (_state == STATE_CONFIRMING || _state == STATE_DECLINED) {
            return;
        }
        if (_state == STATE_FAILED) {
            dc.setColor(Ui.WARNING, Graphics.COLOR_TRANSPARENT);
            dc.drawText(cx, h * 0.44, Graphics.FONT_MEDIUM,
                Ui.wrap(_message != null ? _message : "", Graphics.FONT_MEDIUM, w * 0.8, h * 0.4), center);
            dc.setColor(Ui.QUIET, Graphics.COLOR_TRANSPARENT);
            dc.drawText(cx, h * 0.74, Graphics.FONT_SMALL, Ui.str(Rez.Strings.TryAgain), center);
            return;
        }
        // Waiting: the code, big, as 1234 5678.
        var code = _code != null ? _code : "";
        var font = Ui.fontFor(dc,
            [Graphics.FONT_NUMBER_MEDIUM, Graphics.FONT_NUMBER_MILD, Graphics.FONT_LARGE] as Array<Graphics.FontType>,
            h * 0.2);
        var gap = w * 0.05;
        var y = h * 0.4;
        dc.drawText(cx - gap / 2, y, font, code.substring(0, 4),
            Graphics.TEXT_JUSTIFY_RIGHT | Graphics.TEXT_JUSTIFY_VCENTER);
        dc.drawText(cx + gap / 2, y, font, code.substring(4, 8),
            Graphics.TEXT_JUSTIFY_LEFT | Graphics.TEXT_JUSTIFY_VCENTER);
        dc.setColor(Ui.QUIET, Graphics.COLOR_TRANSPARENT);
        dc.drawText(cx, h * 0.64, Graphics.FONT_TINY,
            Ui.wrap(Ui.str(Rez.Strings.PairingHint), Graphics.FONT_TINY, w * 0.74, h * 0.2), center);
        if (_message != null) {
            dc.setColor(Ui.WARNING, Graphics.COLOR_TRANSPARENT);
            dc.drawText(cx, h * 0.84, Graphics.FONT_XTINY,
                Ui.fit(_message, Graphics.FONT_XTINY, Ui.widthAt(dc, h * 0.86), dc), center);
        }
    }
}

class PairingDelegate extends WatchUi.BehaviorDelegate {
    private var _view as PairingView;

    function initialize(view as PairingView) {
        BehaviorDelegate.initialize();
        _view = view;
    }

    // START after an error: try again with a new code.
    function onSelect() as Boolean {
        if (_view.canRetry()) {
            _view.start();
        }
        return true;
    }

    // BACK leaves; whatever arrives afterwards is ignored.
    function onBack() as Boolean {
        _view.close();
        WatchUi.popView(WatchUi.SLIDE_RIGHT);
        return true;
    }
}

// "Use these rifles?"
class UseRiflesDelegate extends WatchUi.ConfirmationDelegate {
    private var _view as PairingView;

    function initialize(view as PairingView) {
        ConfirmationDelegate.initialize();
        _view = view;
    }

    function onResponse(response as WatchUi.Confirm) as Boolean {
        _view.answer(response == WatchUi.CONFIRM_YES);
        return true;
    }
}
