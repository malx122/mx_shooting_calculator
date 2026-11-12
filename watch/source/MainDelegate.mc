// The main screen's buttons and taps, and the menu.
import Toybox.Lang;
import Toybox.WatchUi;

class MainDelegate extends WatchUi.BehaviorDelegate {
    private var _view as MainView;

    function initialize(view as MainView) {
        BehaviorDelegate.initialize();
        _view = view;
    }

    // START: the wind and the weather; with no rifles, Pair with phone.
    function onSelect() as Boolean {
        Updater.seen();
        if (_view.hasProfile()) {
            openConditions(_view);
        } else {
            openPairing();
        }
        return true;
    }

    // DOWN: a shorter range.
    function onNextPage() as Boolean {
        _view.changeRange(-1);
        return true;
    }

    // UP: a longer range.
    function onPreviousPage() as Boolean {
        _view.changeRange(1);
        return true;
    }

    // A tap on a value sets it; on the rifle's name, lists the rifles (when
    // there are several); anywhere else, as START.
    function onTap(event as WatchUi.ClickEvent) as Boolean {
        if (!_view.hasProfile()) {
            openPairing();
            return true;
        }
        Updater.seen();
        var at = event.getCoordinates();
        var field = _view.fieldAt(at[0], at[1]);
        if (field == MainView.FIELD_NAME && Store.names().size() > 1) {
            openRifles(null);
        } else if (field == MainView.FIELD_NAME) {
            openConditions(_view);
        } else if (field != null) {
            openEdit(_view, field, null);
        } else {
            openConditions(_view);
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

// `chosen` is told when a rifle was chosen.
function openRifles(chosen as Method() as Void or Null) as Void {
    WatchUi.pushView(riflesMenu(), new RiflesDelegate(chosen), WatchUi.SLIDE_LEFT);
}

// START on the main screen: the wind, the weather and (with more than one)
// the rifle, each with its value; START on one sets it.
function openConditions(main as MainView) as Void {
    var menu = new WatchUi.Menu2({ :title => Rez.Strings.Conditions });
    var fields = [MainView.FIELD_SPEED, MainView.FIELD_CLOCK, MainView.FIELD_TEMPERATURE] as Array<Number>;
    if (main.manualPressure()) {
        fields.add(MainView.FIELD_PRESSURE);
    }
    for (var i = 0; i < fields.size(); i++) {
        var field = fields[i];
        menu.addItem(new WatchUi.MenuItem(MainView.titleOf(field), main.textOf(field, main.valueOf(field)), field, null));
    }
    if (Store.names().size() > 1) {
        var profile = Store.profile();
        menu.addItem(new WatchUi.MenuItem(Rez.Strings.Rifle, profile != null ? profile.name : null, :rifle, null));
    }
    WatchUi.pushView(menu, new ConditionsDelegate(main, menu), WatchUi.SLIDE_LEFT);
}

class ConditionsDelegate extends WatchUi.Menu2InputDelegate {
    private var _main as MainView;
    private var _menu as WatchUi.Menu2;
    // The item being set, to show its new value.
    private var _item as WatchUi.MenuItem or Null = null;

    function initialize(main as MainView, menu as WatchUi.Menu2) {
        Menu2InputDelegate.initialize();
        _main = main;
        _menu = menu;
    }

    function onSelect(item as WatchUi.MenuItem) as Void {
        var id = item.getId();
        if (id == :rifle) {
            openRifles(method(:onRifle));
            return;
        }
        // An update from the phone may have come in since the list opened.
        onRifle();
        _item = item;
        openEdit(_main, id as Number, method(:onKept));
    }

    function onKept(text as String) as Void {
        var item = _item;
        if (item != null) {
            item.setSubLabel(text);
        }
    }

    // Another rifle, or an update: every value again, in its units.
    function onRifle() as Void {
        _main.refresh();
        var profile = Store.profile();
        for (var i = 0; ; i++) {
            var item = _menu.getItem(i);
            if (item == null) {
                break;
            }
            var id = item.getId();
            if (id == :rifle) {
                item.setSubLabel(profile != null ? profile.name : null);
            } else {
                item.setSubLabel(_main.textOf(id as Number, _main.valueOf(id as Number)));
            }
        }
        WatchUi.requestUpdate();
    }
}

class MainMenuDelegate extends WatchUi.Menu2InputDelegate {
    function initialize() {
        Menu2InputDelegate.initialize();
    }

    function onSelect(item as WatchUi.MenuItem) as Void {
        var id = item.getId();
        if (id == :rifle) {
            WatchUi.switchToView(riflesMenu(), new RiflesDelegate(null), WatchUi.SLIDE_LEFT);
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
    private var _chosen as Method() as Void or Null;

    function initialize(chosen as Method() as Void or Null) {
        Menu2InputDelegate.initialize();
        _chosen = chosen;
    }

    function onSelect(item as WatchUi.MenuItem) as Void {
        Store.choose(item.getId() as Number);
        WatchUi.popView(WatchUi.SLIDE_RIGHT);
        var chosen = _chosen;
        if (chosen != null) {
            chosen.invoke();
        }
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
