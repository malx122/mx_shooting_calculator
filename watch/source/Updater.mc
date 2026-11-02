// Picks up what the phone has sent since: when the app opens, and on
// Update from phone. Only from the phone the watch is paired with (its key
// is kept), so it asks nothing; it tells the server what it made of it.
import Toybox.Lang;
import Toybox.WatchUi;

module Updater {
    enum State {
        STATE_IDLE,
        STATE_UPDATING,
        // What happened, for the main screen to say once.
        STATE_UPDATED,
        STATE_CURRENT,
        STATE_UNPAIRED,
        STATE_UNREADABLE,
        STATE_FAILED
    }

    var state as Number = STATE_IDLE;
    // Asked for on the menu: say so when there's nothing new or no phone.
    var asked as Boolean = false;
    var fetcher as Fetcher or Null = null;
    // The link the update was started for.
    var token as String or Null = null;

    function start(byHand as Boolean) as Void {
        var link = Store.link();
        if (link == null || state == STATE_UPDATING) {
            return;
        }
        asked = byHand;
        state = STATE_UPDATING;
        token = link["token"] as String;
        var current = new Fetcher(link["token"] as String, Seal.keyPairFromStored(link["secret"] as ByteArray),
            link["app"] as ByteArray, Store.version(), new Lang.Method(Updater, :onFetched));
        fetcher = current;
        current.start();
        WatchUi.requestUpdate();
    }

    // Drops an update under way (a new pairing is starting).
    function stop() as Void {
        var current = fetcher;
        if (current != null) {
            current.stop();
        }
        fetcher = null;
        if (state == STATE_UPDATING) {
            state = STATE_IDLE;
        }
    }

    function onFetched(result as Number) as Void {
        var current = fetcher;
        fetcher = null;
        var started = token;
        // Only for the link it was started for: a pairing since replaces it.
        if (current == null || started == null || !started.equals(Store.token())) {
            state = STATE_IDLE;
            return;
        }
        if (result == Fetcher.RESULT_FETCHED) {
            if (Store.commit(current.slot, current.version, current.names, current.ids)) {
                Outbox.done(started, "used", current.version);
                state = STATE_UPDATED;
            } else {
                Outbox.done(started, "unreadable", current.version);
                state = STATE_UNREADABLE;
            }
        } else if (result == Fetcher.RESULT_SAME) {
            state = asked ? STATE_CURRENT : STATE_IDLE;
        } else if (result == Fetcher.RESULT_GONE) {
            // Forgotten by the phone, or unused for over a year: the
            // profiles stay, the link goes.
            Store.forgetLink();
            state = STATE_UNPAIRED;
        } else if (result == Fetcher.RESULT_UNREADABLE) {
            Outbox.done(started, "unreadable", current.version);
            state = STATE_UNREADABLE;
        } else {
            state = asked ? STATE_FAILED : STATE_IDLE;
        }
        WatchUi.requestUpdate();
    }

    // What to say about it on the main screen, or null.
    function note() as String or Null {
        if (state == STATE_UPDATING) {
            return Ui.str(Rez.Strings.Updating);
        }
        if (state == STATE_UPDATED) {
            return Ui.str(Rez.Strings.Updated);
        }
        if (state == STATE_CURRENT) {
            return Ui.str(Rez.Strings.UpToDate);
        }
        if (state == STATE_UNPAIRED) {
            return Ui.str(Rez.Strings.Unpaired);
        }
        if (state == STATE_UNREADABLE) {
            return Ui.str(Rez.Strings.UpdateUnreadable);
        }
        if (state == STATE_FAILED) {
            return Ui.str(Rez.Strings.NoPhone);
        }
        return null;
    }

    // The note has been seen: a key press clears it.
    function seen() as Void {
        if (state != STATE_UPDATING) {
            state = STATE_IDLE;
        }
    }
}
