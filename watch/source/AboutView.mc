// About this rifle: when the phone sent it, the ethical limit in this
// weather, how far it goes, and where the pressure comes from.
import Toybox.Graphics;
import Toybox.Lang;
import Toybox.Math;
import Toybox.Time;
import Toybox.Time.Gregorian;
import Toybox.WatchUi;

class AboutView extends WatchUi.View {
    private var _profile as Profile or Null;

    function initialize() {
        View.initialize();
    }

    function onShow() as Void {
        _profile = Store.profile();
    }

    function onUpdate(dc as Graphics.Dc) as Void {
        dc.setColor(Ui.TEXT, Ui.BACKGROUND);
        dc.clear();
        var cx = dc.getWidth() / 2;
        var h = dc.getHeight();
        var center = Graphics.TEXT_JUSTIFY_CENTER | Graphics.TEXT_JUSTIFY_VCENTER;
        var profile = _profile;
        if (profile == null) {
            dc.drawText(cx, h / 2, Graphics.FONT_MEDIUM, Ui.str(Rez.Strings.NoRifles), center);
            return;
        }
        dc.drawText(cx, h * 0.22, Graphics.FONT_TINY,
            Ui.wrap(profile.name, Graphics.FONT_TINY, dc.getWidth() * 0.72, h * 0.18), center);

        var made = Gregorian.info(new Time.Moment(profile.version), Time.FORMAT_SHORT);
        var date = Lang.format("$1$-$2$-$3$", [made.year, (made.month as Number).format("%02d"), made.day.format("%02d")]);
        var unit = Ui.rangeUnit(profile.rangeUnit);
        var perUnit = Units.metresPer(profile.rangeUnit);
        var limit = profile.ethicalLimit(Weather.temperatureC(), Weather.pressureHpa());
        var ethical = limit != null
            ? Lang.format(Ui.str(Rez.Strings.EthicalLimit), [Math.round(limit / perUnit).toNumber() + " " + unit])
            : Ui.str(Rez.Strings.NoEthicalLimit);
        var last = Lang.format(Ui.str(Rez.Strings.LastRange),
            [Math.floor(profile.lastM() / perUnit + 1.0e-6).toNumber() + " " + unit]);
        var lines = [
            Lang.format(Ui.str(Rez.Strings.MadeAt), [date]),
            ethical,
            last,
            Ui.str(Weather.barometerHpa() != null ? Rez.Strings.PressureFromBarometer : Rez.Strings.PressureByHand),
            Ui.str(Store.link() != null ? Rez.Strings.Paired : Rez.Strings.NotPaired)
        ];
        var y = h * 0.4;
        for (var i = 0; i < lines.size(); i++) {
            dc.setColor(i == 1 && limit != null ? Ui.TEXT : Ui.QUIET, Graphics.COLOR_TRANSPARENT);
            dc.drawText(cx, y, Graphics.FONT_SMALL,
                Ui.fit(lines[i], Graphics.FONT_SMALL, Ui.widthAt(dc, y + 20), dc), center);
            y += h * 0.1;
        }
    }
}
