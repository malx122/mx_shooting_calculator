// Sets one value the Garmin way: UP and DOWN change it, START keeps it,
// BACK leaves it as it was.
import Toybox.Graphics;
import Toybox.Lang;
import Toybox.System;
import Toybox.WatchUi;

class EditView extends WatchUi.View {
    private var _main as MainView;
    private var _field as Number;
    private var _value as Double;
    // As it was when opened: kept only when changed.
    private var _start as Double;
    // Told the value kept (a list showing it updates).
    private var _kept as Method(text as String) as Void or Null;

    function initialize(main as MainView, field as Number, kept as Method(text as String) as Void or Null) {
        View.initialize();
        _main = main;
        _field = field;
        _value = main.valueOf(field);
        _start = _value;
        _kept = kept;
    }

    // UP (+1) or DOWN (-1).
    function change(direction as Number) as Void {
        _value = _main.stepped(_field, _value, direction);
        WatchUi.requestUpdate();
    }

    function keep() as Void {
        if (_value == _start) {
            return;
        }
        _main.set(_field, _value);
        var kept = _kept;
        if (kept != null) {
            kept.invoke(_main.textOf(_field, _value));
        }
    }

    function onUpdate(dc as Graphics.Dc) as Void {
        dc.setColor(Ui.TEXT, Ui.BACKGROUND);
        dc.clear();
        var cx = dc.getWidth() / 2;
        var h = dc.getHeight();

        var titleY = h * 0.2;
        dc.setColor(Ui.QUIET, Graphics.COLOR_TRANSPARENT);
        dc.drawText(cx, titleY, Graphics.FONT_SMALL,
            Ui.fit(Ui.str(MainView.titleOf(_field)), Graphics.FONT_SMALL, Ui.widthAt(dc, titleY), dc),
            Graphics.TEXT_JUSTIFY_CENTER | Graphics.TEXT_JUSTIFY_VCENTER);

        var valueY = h * 0.5;
        var text = _main.textOf(_field, _value);
        var font = Graphics.FONT_SMALL;
        var fonts = [Graphics.FONT_LARGE, Graphics.FONT_MEDIUM] as Array<Graphics.FontType>;
        for (var i = 0; i < fonts.size(); i++) {
            if (dc.getTextWidthInPixels(text, fonts[i]) <= Ui.widthAt(dc, valueY)) {
                font = fonts[i];
                break;
            }
        }
        dc.setColor(Ui.TEXT, Graphics.COLOR_TRANSPARENT);
        dc.drawText(cx, valueY, font, text, Graphics.TEXT_JUSTIFY_CENTER | Graphics.TEXT_JUSTIFY_VCENTER);

        // Where UP and DOWN take it.
        var gap = dc.getFontHeight(font) / 2 + h * 0.06;
        Ui.arrows(dc, cx, valueY, gap, h * 0.035, Ui.FOCUS);
    }
}

class EditDelegate extends WatchUi.BehaviorDelegate {
    private var _view as EditView;

    function initialize(view as EditView) {
        BehaviorDelegate.initialize();
        _view = view;
    }

    // UP.
    function onPreviousPage() as Boolean {
        _view.change(1);
        return true;
    }

    // DOWN.
    function onNextPage() as Boolean {
        _view.change(-1);
        return true;
    }

    // START keeps the value.
    function onSelect() as Boolean {
        _view.keep();
        WatchUi.popView(WatchUi.SLIDE_RIGHT);
        return true;
    }

    // A tap on the upper arrow raises it, on the lower one lowers it; on
    // the value keeps it.
    function onTap(event as WatchUi.ClickEvent) as Boolean {
        var y = event.getCoordinates()[1];
        var h = System.getDeviceSettings().screenHeight;
        if (y < h * 0.4) {
            _view.change(1);
        } else if (y > h * 0.6) {
            _view.change(-1);
        } else {
            onSelect();
        }
        return true;
    }

    // BACK leaves it as it was.
    function onBack() as Boolean {
        WatchUi.popView(WatchUi.SLIDE_RIGHT);
        return true;
    }
}

function openEdit(main as MainView, field as Number, kept as Method(text as String) as Void or Null) as Void {
    var view = new EditView(main, field, kept);
    WatchUi.pushView(view, new EditDelegate(view), WatchUi.SLIDE_LEFT);
}
