// The main screen: range, wind and temperature in, clicks and centimetres
// out, for the chosen rifle and load.
import Toybox.Graphics;
import Toybox.Lang;
import Toybox.Math;
import Toybox.WatchUi;

class MainView extends WatchUi.View {
    // The values set on the main screen; UP and DOWN change the range, the
    // others are set in their own view (START, or a tap on them).
    enum Field {
        FIELD_RANGE,
        FIELD_SPEED,
        FIELD_CLOCK,
        FIELD_TEMPERATURE,
        // Only when the barometer gives no reading.
        FIELD_PRESSURE,
        // The rifle's name: a tap opens the list.
        FIELD_NAME
    }
    const FIELDS = 5;
    const RANGE_STEP = 10;
    const MAX_WIND_MPS = 30.0d;
    const MIN_C = -45.0d;
    const MAX_C = 50.0d;
    const MIN_HPA = 500.0d;
    const MAX_HPA = 1100.0d;

    private var _profile as Profile or Null;
    // In the profile's range unit.
    private var _range as Number = 100;
    // In the profile's wind unit.
    private var _wind as Double = 0.0d;
    private var _clock as Number = 3;
    // Where each field was last drawn: [left, top, right, bottom].
    private var _boxes as Array<Array<Number> or Null> = new Array<Array<Number> or Null>[FIELDS + 1];

    function initialize() {
        View.initialize();
    }

    function onShow() as Void {
        refresh();
        WatchUi.requestUpdate();
    }

    // Picks up an update, or another rifle chosen, and then reads the
    // settings again in its units.
    function refresh() as Void {
        var latest = Store.profile();
        if (latest != _profile) {
            _profile = latest;
            load();
        }
    }

    // Reads the last settings, fitted to the profile's units.
    private function load() as Void {
        var profile = _profile;
        if (profile == null) {
            return;
        }
        var range = Store.number(Store.RANGE);
        var rangeUnit = Store.number(Store.RANGE_UNIT);
        if (range == null || rangeUnit == null) {
            range = 100.0d / Units.metresPer(profile.rangeUnit);
        } else if (rangeUnit.toNumber() != profile.rangeUnit) {
            range = range.toDouble() * Units.metresPer(rangeUnit.toNumber()) / Units.metresPer(profile.rangeUnit);
        }
        _range = clampRange((Math.round(range.toDouble() / RANGE_STEP) * RANGE_STEP).toNumber());
        var wind = Store.number(Store.WIND);
        var windUnit = Store.number(Store.WIND_UNIT);
        if (wind == null || windUnit == null) {
            wind = 0.0d;
        } else if (windUnit.toNumber() != profile.windUnit) {
            wind = wind.toDouble() * Units.mpsPer(windUnit.toNumber()) / Units.mpsPer(profile.windUnit);
        }
        var step = windStep();
        _wind = clampWind(Math.round(wind.toDouble() / step) * step);
        var clock = Store.number(Store.CLOCK);
        _clock = clock != null && clock >= 1 && clock <= 12 ? clock.toNumber() : 3;
    }

    function hasProfile() as Boolean {
        return _profile != null;
    }

    // The pressure is set by hand when the barometer gives none.
    function manualPressure() as Boolean {
        return Weather.barometerHpa() == null;
    }

    // UP (+1) or DOWN (-1): the range.
    function changeRange(direction as Number) as Void {
        Updater.seen();
        if (_profile == null) {
            return;
        }
        set(FIELD_RANGE, stepped(FIELD_RANGE, _range.toDouble(), direction));
    }

    // What a field is called, over its value while it is set.
    static function titleOf(field as Number) as ResourceId {
        if (field == FIELD_RANGE) {
            return Rez.Strings.Range;
        }
        if (field == FIELD_SPEED) {
            return Rez.Strings.Wind;
        }
        if (field == FIELD_CLOCK) {
            return Rez.Strings.WindFrom;
        }
        if (field == FIELD_TEMPERATURE) {
            return Rez.Strings.Temperature;
        }
        return Rez.Strings.AirPressure;
    }

    // A field's value as it is set: the range and the wind in the
    // profile's units, the clock 1 to 12, whole degrees in the hunter's
    // unit, hPa.
    function valueOf(field as Number) as Double {
        var profile = _profile;
        if (field == FIELD_RANGE) {
            return _range.toDouble();
        }
        if (field == FIELD_SPEED) {
            return _wind;
        }
        if (field == FIELD_CLOCK) {
            return _clock.toDouble();
        }
        if (field == FIELD_TEMPERATURE) {
            var unit = profile != null ? profile.temperatureUnit : Units.TEMPERATURE_C;
            return Math.round(Units.fromCelsius(Weather.temperatureC(), unit)).toDouble();
        }
        return Weather.pressureHpa();
    }

    // One step up (+1) or down (-1) from `value`, kept within bounds.
    function stepped(field as Number, value as Double, direction as Number) as Double {
        var profile = _profile;
        if (profile == null) {
            return value;
        }
        if (field == FIELD_RANGE) {
            return clampRange(value.toNumber() + direction * RANGE_STEP).toDouble();
        }
        if (field == FIELD_SPEED) {
            return clampWind(value + direction * windStep());
        }
        if (field == FIELD_CLOCK) {
            return ((value.toNumber() - 1 + direction + 12) % 12 + 1).toDouble();
        }
        if (field == FIELD_TEMPERATURE) {
            var next = value + direction;
            var celsius = Units.toCelsius(next, profile.temperatureUnit);
            return celsius < MIN_C || celsius > MAX_C ? value : next;
        }
        var hpa = value + direction * pressureStep(profile);
        return hpa < MIN_HPA ? MIN_HPA : hpa > MAX_HPA ? MAX_HPA : hpa;
    }

    // A field's value with its unit.
    function textOf(field as Number, value as Double) as String {
        var profile = _profile;
        if (profile == null) {
            return "";
        }
        if (field == FIELD_RANGE) {
            return value.toNumber() + " " + Ui.rangeUnit(profile.rangeUnit);
        }
        if (field == FIELD_SPEED) {
            return Ui.speed(value) + " " + Ui.windUnit(profile.windUnit);
        }
        if (field == FIELD_CLOCK) {
            return Lang.format(Ui.str(Rez.Strings.Clock), [value.toNumber()]);
        }
        if (field == FIELD_TEMPERATURE) {
            return value.toNumber() + " " + Ui.temperatureUnit(profile.temperatureUnit);
        }
        return Ui.pressure(value, profile.pressureUnit);
    }

    // Keeps a field's value, here and for next time.
    function set(field as Number, value as Double) as Void {
        var profile = _profile;
        if (profile == null) {
            return;
        }
        if (field == FIELD_RANGE) {
            _range = value.toNumber();
            Store.save(Store.RANGE, _range);
            Store.save(Store.RANGE_UNIT, profile.rangeUnit);
        } else if (field == FIELD_SPEED) {
            _wind = value;
            Store.save(Store.WIND, _wind.toFloat());
            Store.save(Store.WIND_UNIT, profile.windUnit);
        } else if (field == FIELD_CLOCK) {
            _clock = value.toNumber();
            Store.save(Store.CLOCK, _clock);
        } else if (field == FIELD_TEMPERATURE) {
            var celsius = Units.toCelsius(value, profile.temperatureUnit);
            Weather.setTemperatureC(celsius < MIN_C ? MIN_C : celsius > MAX_C ? MAX_C : celsius);
        } else if (field == FIELD_PRESSURE) {
            Weather.setPressureHpa(value);
        }
        WatchUi.requestUpdate();
    }

    // Steps of about 5 hPa, round in the hunter's unit.
    private function pressureStep(profile as Profile) as Double {
        if (profile.pressureUnit == Units.PRESSURE_INHG) {
            return 0.15d * Units.hpaPer(Units.PRESSURE_INHG);
        }
        if (profile.pressureUnit == Units.PRESSURE_MMHG) {
            return 4.0d * Units.hpaPer(Units.PRESSURE_MMHG);
        }
        return 5.0d;
    }

    // The field drawn at x, y, or null.
    function fieldAt(x as Number, y as Number) as Number or Null {
        var slack = 16;
        for (var i = 0; i < _boxes.size(); i++) {
            var box = _boxes[i];
            if (box != null && x >= box[0] - slack && x <= box[2] + slack
                    && y >= box[1] - slack && y <= box[3] + slack) {
                return i;
            }
        }
        return null;
    }

    private function windStep() as Double {
        var profile = _profile;
        return profile != null && profile.windUnit == Units.WIND_MPS ? 0.5d : 1.0d;
    }

    private function clampRange(range as Number) as Number {
        var profile = _profile;
        var last = RANGE_STEP;
        var first = RANGE_STEP;
        if (profile != null) {
            var perUnit = Units.metresPer(profile.rangeUnit);
            var inUnit = (profile.lastM() / perUnit + 1.0e-6d).toNumber();
            last = inUnit - inUnit % RANGE_STEP;
            // The first row, rounded up to a step.
            var start = Math.ceil(profile.step / perUnit / RANGE_STEP - 1.0e-6d).toNumber() * RANGE_STEP;
            first = start > RANGE_STEP ? start : RANGE_STEP;
        }
        if (last < first) {
            last = first;
        }
        return range < first ? first : range > last ? last : range;
    }

    private function clampWind(wind as Numeric) as Double {
        var profile = _profile;
        var unit = profile != null ? profile.windUnit : Units.WIND_MPS;
        var step = windStep();
        var max = Math.floor(MAX_WIND_MPS / Units.mpsPer(unit) / step + 1.0e-6d) * step;
        var value = wind.toDouble();
        return value < 0.0d ? 0.0d : value > max ? max.toDouble() : value;
    }

    function onUpdate(dc as Graphics.Dc) as Void {
        dc.setColor(Ui.TEXT, Ui.BACKGROUND);
        dc.clear();
        _boxes = new Array<Array<Number> or Null>[FIELDS + 1];
        // An update, or another rifle chosen, while the screen was up.
        refresh();
        var profile = _profile;
        if (profile == null) {
            drawNoProfile(dc);
            return;
        }
        var w = dc.getWidth();
        var h = dc.getHeight();
        var cx = w / 2;
        var rangeM = _range * Units.metresPer(profile.rangeUnit);
        var temperatureC = Weather.temperatureC();
        var pressureHpa = Weather.pressureHpa();
        var limit = profile.ethicalLimit(temperatureC, pressureHpa);
        var pastEthical = limit != null && rangeM > limit + 1.0e-6d;
        var outside = profile.outsideGrid(temperatureC, pressureHpa);

        // The rifle and load, small; a tap lists the others.
        var nameY = h * 0.13;
        var name = Ui.fit(profile.name, Graphics.FONT_XTINY, Ui.widthAt(dc, nameY - 12), dc);
        dc.setColor(Ui.QUIET, Graphics.COLOR_TRANSPARENT);
        dc.drawText(cx, nameY, Graphics.FONT_XTINY, name,
            Graphics.TEXT_JUSTIFY_CENTER | Graphics.TEXT_JUSTIFY_VCENTER);
        var nameHalf = dc.getTextWidthInPixels(name, Graphics.FONT_XTINY) / 2;
        var nameHeight = dc.getFontHeight(Graphics.FONT_XTINY) / 2;
        _boxes[FIELD_NAME] = [cx - nameHalf, (nameY - nameHeight).toNumber(), cx + nameHalf, (nameY + nameHeight).toNumber()];

        // The range, big.
        var rangeY = h * 0.26;
        var rangeColor = pastEthical ? Ui.WARNING : Ui.TEXT;
        var rangeFont = Ui.fontFor(dc,
            [Graphics.FONT_NUMBER_MEDIUM, Graphics.FONT_NUMBER_MILD, Graphics.FONT_LARGE] as Array<Graphics.FontType>,
            h * 0.15);
        var spans = Ui.drawPieces(dc, cx, rangeY, [
            [_range.toString(), rangeFont, rangeColor],
            [" " + Ui.rangeUnit(profile.rangeUnit), Graphics.FONT_SMALL, rangeColor]
        ] as Array<Array>);
        var rangeHalf = dc.getFontHeight(rangeFont) / 2;
        // UP and DOWN change it; a tap on the arrows too sets it.
        var arrowsX = spans[0][0] - h * 0.05;
        Ui.arrows(dc, arrowsX, rangeY, h * 0.03, h * 0.022, Ui.QUIET);
        _boxes[FIELD_RANGE] = [(arrowsX - h * 0.022).toNumber(), (rangeY - rangeHalf).toNumber(), spans[1][1], (rangeY + rangeHalf).toNumber()];

        // The wind: speed and clock.
        var windY = h * 0.385;
        var speedText = Ui.speed(_wind) + " " + Ui.windUnit(profile.windUnit);
        var clockText = Lang.format(Ui.str(Rez.Strings.Clock), [_clock]);
        var windFont = fitting(dc, speedText + " · " + clockText, h * 0.09, Ui.widthAt(dc, windY + h * 0.05),
            [Graphics.FONT_MEDIUM, Graphics.FONT_SMALL, Graphics.FONT_TINY] as Array<Graphics.FontType>);
        spans = Ui.drawPieces(dc, cx, windY, [
            [speedText, windFont, Ui.TEXT],
            [" · ", windFont, Ui.QUIET],
            [clockText, windFont, Ui.TEXT]
        ] as Array<Array>);
        var windHalf = dc.getFontHeight(windFont) / 2;
        var windTop = (windY - windHalf).toNumber();
        var windBottom = (windY + windHalf).toNumber();
        _boxes[FIELD_SPEED] = [spans[0][0], windTop, spans[0][1], windBottom];
        _boxes[FIELD_CLOCK] = [spans[2][0], windTop, spans[2][1], windBottom];

        // The weather, at the foot.
        var weatherY = h * 0.935;
        var temperatureText = Math.round(Units.fromCelsius(temperatureC, profile.temperatureUnit)).toNumber()
            + " " + Ui.temperatureUnit(profile.temperatureUnit);
        var pressureText = Ui.pressure(pressureHpa, profile.pressureUnit);
        var weatherColor = outside ? Ui.WARNING : Ui.TEXT;
        spans = Ui.drawPieces(dc, cx, weatherY, [
            [temperatureText, Graphics.FONT_XTINY, weatherColor],
            [" · ", Graphics.FONT_XTINY, Ui.QUIET],
            [pressureText, Graphics.FONT_XTINY, manualPressure() ? weatherColor : Ui.QUIET]
        ] as Array<Array>);
        var weatherHalf = dc.getFontHeight(Graphics.FONT_XTINY) / 2;
        var weatherTop = (weatherY - weatherHalf).toNumber();
        var weatherBottom = (weatherY + weatherHalf).toNumber();
        _boxes[FIELD_TEMPERATURE] = [spans[0][0], weatherTop, spans[0][1], weatherBottom];
        if (manualPressure()) {
            _boxes[FIELD_PRESSURE] = [spans[2][0], weatherTop, spans[2][1], weatherBottom];
        }

        // One note: the ethical limit first, then weather past the grid,
        // then news from the phone.
        var note = null;
        var noteColor = Ui.WARNING;
        if (pastEthical) {
            note = Ui.str(Rez.Strings.PastEthical);
        } else if (outside) {
            note = Ui.str(Rez.Strings.OutsideWeather);
        } else {
            note = Updater.note();
            noteColor = Ui.QUIET;
        }
        if (note != null) {
            var noteY = h * 0.475;
            dc.setColor(noteColor, Graphics.COLOR_TRANSPARENT);
            dc.drawText(cx, noteY, Graphics.FONT_XTINY, Ui.fit(note, Graphics.FONT_XTINY, Ui.widthAt(dc, noteY), dc),
                Graphics.TEXT_JUSTIFY_CENTER | Graphics.TEXT_JUSTIFY_VCENTER);
        }

        var hold = profile.hold(rangeM, _wind * Units.mpsPer(profile.windUnit), _clock, temperatureC, pressureHpa);
        if (hold == null) {
            dc.setColor(Ui.QUIET, Graphics.COLOR_TRANSPARENT);
            dc.drawText(cx, h * 0.66, Graphics.FONT_MEDIUM, Ui.str(Rez.Strings.BeyondTable),
                Graphics.TEXT_JUSTIFY_CENTER | Graphics.TEXT_JUSTIFY_VCENTER);
            return;
        }
        drawHold(dc, profile, hold.elevationMrad, rangeM, h * 0.565, h * 0.655,
            Rez.Strings.Up, Rez.Strings.Down);
        drawHold(dc, profile, hold.windageMrad, rangeM, h * 0.745, h * 0.835,
            Rez.Strings.Right, Rez.Strings.Left);
    }

    // The largest of `fonts` that fits `text` in `height` and `width`.
    private function fitting(dc as Graphics.Dc, text as String, height as Numeric, width as Numeric,
            fonts as Array<Graphics.FontType>) as Graphics.FontType {
        for (var i = 0; i < fonts.size(); i++) {
            if (dc.getFontHeight(fonts[i]) <= height && dc.getTextWidthInPixels(text, fonts[i]) <= width) {
                return fonts[i];
            }
        }
        return Graphics.FONT_XTINY;
    }

    // One result: "UP 12" big, the angle and the length at the target under it.
    private function drawHold(dc as Graphics.Dc, profile as Profile, mrad as Double, rangeM as Double,
            y as Numeric, detailY as Numeric, positive as ResourceId, negative as ResourceId) as Void {
        var h = dc.getHeight();
        var clicks = profile.clicks(mrad);
        var numberFont = Ui.fontFor(dc,
            [Graphics.FONT_NUMBER_MEDIUM, Graphics.FONT_NUMBER_MILD, Graphics.FONT_LARGE] as Array<Graphics.FontType>,
            h * 0.12);
        var pieces = [] as Array<Array>;
        if (clicks != 0) {
            pieces.add([Ui.str(clicks > 0 ? positive : negative) + " ", Graphics.FONT_MEDIUM, Ui.TEXT]);
        }
        pieces.add([clicks.abs().toString(), numberFont, Ui.TEXT]);
        Ui.drawPieces(dc, dc.getWidth() / 2, y, pieces);

        var angle = profile.inClickUnit(mrad).abs();
        var length = profile.atTarget(mrad, rangeM).abs();
        var detail = Ui.decimal(angle, profile.clickUnit == Units.CLICK_MOA ? 1 : 2) + " "
            + Ui.clickUnit(profile.clickUnit) + " · "
            + Ui.decimal(length, profile.lengthUnit == Units.LENGTH_IN ? 1 : 0) + " "
            + Ui.lengthUnit(profile.lengthUnit);
        dc.setColor(Ui.QUIET, Graphics.COLOR_TRANSPARENT);
        dc.drawText(dc.getWidth() / 2, detailY, Graphics.FONT_TINY,
            Ui.fit(detail, Graphics.FONT_TINY, Ui.widthAt(dc, detailY + 15), dc),
            Graphics.TEXT_JUSTIFY_CENTER | Graphics.TEXT_JUSTIFY_VCENTER);
    }

    private function drawNoProfile(dc as Graphics.Dc) as Void {
        var cx = dc.getWidth() / 2;
        var h = dc.getHeight();
        dc.setColor(Ui.TEXT, Graphics.COLOR_TRANSPARENT);
        dc.drawText(cx, h * 0.42, Graphics.FONT_MEDIUM, Ui.str(Rez.Strings.NoRifles),
            Graphics.TEXT_JUSTIFY_CENTER | Graphics.TEXT_JUSTIFY_VCENTER);
        dc.setColor(Ui.QUIET, Graphics.COLOR_TRANSPARENT);
        dc.drawText(cx, h * 0.56, Graphics.FONT_SMALL,
            Ui.wrap(Ui.str(Rez.Strings.NoRiflesHint), Graphics.FONT_SMALL, dc.getWidth() * 0.75, h * 0.2),
            Graphics.TEXT_JUSTIFY_CENTER | Graphics.TEXT_JUSTIFY_VCENTER);
        var note = Updater.note();
        if (note != null) {
            dc.drawText(cx, h * 0.75, Graphics.FONT_XTINY,
                Ui.fit(note, Graphics.FONT_XTINY, Ui.widthAt(dc, h * 0.75), dc),
                Graphics.TEXT_JUSTIFY_CENTER | Graphics.TEXT_JUSTIFY_VCENTER);
        }
    }
}
