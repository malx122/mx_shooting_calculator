// MX Shooting Calculator on the watch: the clicks and centimetres for a
// range, a wind and the weather, from the rifles the phone app sends.
import Toybox.Application;
import Toybox.Lang;
import Toybox.WatchUi;

class MxscApp extends Application.AppBase {
    function initialize() {
        AppBase.initialize();
    }

    // Sends what the server hasn't heard yet, and picks up what the phone
    // has sent since.
    function onStart(state as Dictionary?) as Void {
        Outbox.resume();
        Updater.start(false);
    }

    function getInitialView() as [WatchUi.Views] or [WatchUi.Views, WatchUi.InputDelegates] {
        var view = new MainView();
        return [view, new MainDelegate(view)];
    }
}
