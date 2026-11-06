// The main screen's buttons and taps, and the menu.
import Toybox.Lang;
import Toybox.WatchUi;

class MainDelegate extends WatchUi.BehaviorDelegate {
    private var _view as MainView;

    function initialize(view as MainView) {
        BehaviorDelegate.initialize();
        _view = view;
    }

    // START: the next field; with no rifles, Pair with phone.
    function onSelect() as Boolean {
        if (_view.hasProfile()) {
            _view.nextFocus();
        } else {
            openPairing();
        }
        return true;
    }

    // DOWN.
    function onNextPage() as Boolean {
        _view.change(-1);
        return true;
    }

    // UP.
    function onPreviousPage() as Boolean {
        _view.change(1);
        return true;
    }

    // A tap on a field focuses it; on the rifle's name, lists the rifles.
    function onTap(event as WatchUi.ClickEvent) as Boolean {
        if (!_view.hasProfile()) {
            openPairing();
            return true;
        }
        var at = event.getCoordinates();
        var field = _view.fieldAt(at[0], at[1]);
        if (field == MainView.FIELD_NAME) {
            openRifles();
        } else if (field != null) {
            _view.focus(field);
        }
        return true;
    }

    function onMenu() as Boolean {
        var menu = new WatchUi.Menu2({ :title => Rez.Strings.AppName });
        if (Store.names().size() > 1) {
            menu.addItem(new WatchUi.MenuItem(Rez.Strings.Rifle, Store.profile() != null ? (Store.profile() as Profile).name : null, :rifle, null));
        }
        var paired = Store.link() != null;
        // While a pairing is being saved, neither may write its slot.
        if (paired && !Store.saving) {
            menu.addItem(new WatchUi.MenuItem(Rez.Strings.UpdateFromPhone, null, :update, null));
        }
        if (!Store.saving) {
            menu.addItem(new WatchUi.MenuItem(Rez.Strings.PairWithPhone, null, :pair, null));
        }
        if (Store.profile() != null) {
            menu.addItem(new WatchUi.MenuItem(Rez.Strings.AboutRifle, null, :about, null));
        }
        if (paired) {
            menu.addItem(new WatchUi.MenuItem(Rez.Strings.ForgetPhone, null, :forget, null));
        }
        WatchUi.pushView(menu, new MainMenuDelegate(), WatchUi.SLIDE_UP);
        return true;
    }

    // BACK leaves the app.
    function onBack() as Boolean {
        return false;
    }
}

function openPairing() as Void {
    var view = new PairingView();
    WatchUi.pushView(view, new PairingDelegate(view), WatchUi.SLIDE_LEFT);
}

// The rifles and loads on the watch, the chosen one first in focus.
function riflesMenu() as WatchUi.Menu2 {
    var menu = new WatchUi.Menu2({ :title => Rez.Strings.Rifle });
    var names = Store.names();
    for (var i = 0; i < names.size(); i++) {
        menu.addItem(new WatchUi.MenuItem(names[i], null, i, null));
    }
    menu.setFocus(Store.chosenIndex());
    return menu;
}

function openRifles() as Void {
    WatchUi.pushView(riflesMenu(), new RiflesDelegate(), WatchUi.SLIDE_UP);
}

class MainMenuDelegate extends WatchUi.Menu2InputDelegate {
    function initialize() {
        Menu2InputDelegate.initialize();
    }

    function onSelect(item as WatchUi.MenuItem) as Void {
        var id = item.getId();
        if (id == :rifle) {
            WatchUi.switchToView(riflesMenu(), new RiflesDelegate(), WatchUi.SLIDE_LEFT);
        } else if (id == :update) {
            Updater.start(true);
            WatchUi.popView(WatchUi.SLIDE_DOWN);
        } else if (id == :pair) {
            var view = new PairingView();
            WatchUi.switchToView(view, new PairingDelegate(view), WatchUi.SLIDE_LEFT);
        } else if (id == :about) {
            WatchUi.switchToView(new AboutView(), new WatchUi.BehaviorDelegate(), WatchUi.SLIDE_LEFT);
        } else if (id == :forget) {
            WatchUi.switchToView(new WatchUi.Confirmation(Ui.str(Rez.Strings.ForgetPhoneQuestion)),
                new ForgetDelegate(), WatchUi.SLIDE_LEFT);
        }
    }
}

class RiflesDelegate extends WatchUi.Menu2InputDelegate {
    function initialize() {
        Menu2InputDelegate.initialize();
    }

    function onSelect(item as WatchUi.MenuItem) as Void {
        Store.choose(item.getId() as Number);
        WatchUi.popView(WatchUi.SLIDE_DOWN);
    }
}

// "Forget the phone?": the link goes on the watch and the server; the
// rifles stay.
class ForgetDelegate extends WatchUi.ConfirmationDelegate {
    function initialize() {
        ConfirmationDelegate.initialize();
    }

    function onResponse(response as WatchUi.Confirm) as Boolean {
        if (response == WatchUi.CONFIRM_YES) {
            var token = Store.token();
            if (token != null) {
                Outbox.forget(token);
            }
            Store.forgetLink();
        }
        return true;
    }
}
