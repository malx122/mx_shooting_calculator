// Small drawing and text helpers shared by the views.
import Toybox.Graphics;
import Toybox.Lang;
import Toybox.Math;
import Toybox.WatchUi;

module Ui {
    const BACKGROUND = Graphics.COLOR_BLACK;
    const TEXT = Graphics.COLOR_WHITE;
    const QUIET = Graphics.COLOR_LT_GRAY;
    const WARNING = Graphics.COLOR_RED;
    const FOCUS = 0x00AAFF;

    function str(id as ResourceId) as String {
        return WatchUi.loadResource(id) as String;
    }

    // The first font in `fonts` no taller than `height`, else the last.
    function fontFor(dc as Graphics.Dc, fonts as Array<Graphics.FontType>, height as Numeric) as Graphics.FontType {
        for (var i = 0; i < fonts.size() - 1; i++) {
            if (dc.getFontHeight(fonts[i]) <= height) {
                return fonts[i];
            }
        }
        return fonts[fonts.size() - 1];
    }

    // How wide a line can be at height `y` on a round screen, less a margin.
    function widthAt(dc as Graphics.Dc, y as Numeric) as Number {
        var r = dc.getWidth() / 2.0;
        var dy = (y - dc.getHeight() / 2.0).abs();
        var half = dy < r ? Math.sqrt(r * r - dy * dy) : 0.0;
        return (2 * half - 24).toNumber();
    }

    // One line cut to `width`, with ... if it had to be cut.
    function fit(text as String, font as Graphics.FontType, width as Numeric, dc as Graphics.Dc) as String {
        if (dc.getTextWidthInPixels(text, font) <= width) {
            return text;
        }
        var fitted = Graphics.fitTextToArea(text, font, width, dc.getFontHeight(font), true);
        return fitted != null ? fitted : "";
    }

    // Text cut into lines that fit `width` × `height`.
    function wrap(text as String, font as Graphics.FontType, width as Numeric, height as Numeric) as String {
        var fitted = Graphics.fitTextToArea(text, font, width, height, true);
        return fitted != null ? fitted : "";
    }

    // Draws pieces [text, font, colour] side by side, the whole centred on
    // x and y. Gives back each piece's [left, right].
    function drawPieces(dc as Graphics.Dc, x as Numeric, y as Numeric, pieces as Array<Array>) as Array<Array<Number>> {
        var widths = [] as Array<Number>;
        var total = 0;
        for (var i = 0; i < pieces.size(); i++) {
            var width = dc.getTextWidthInPixels(pieces[i][0] as String, pieces[i][1] as Graphics.FontType);
            widths.add(width);
            total += width;
        }
        var left = (x - total / 2.0).toNumber();
        var spans = [] as Array<Array<Number>>;
        for (var i = 0; i < pieces.size(); i++) {
            dc.setColor(pieces[i][2] as Graphics.ColorType, Graphics.COLOR_TRANSPARENT);
            dc.drawText(left, y, pieces[i][1] as Graphics.FontType, pieces[i][0] as String,
                Graphics.TEXT_JUSTIFY_LEFT | Graphics.TEXT_JUSTIFY_VCENTER);
            spans.add([left, left + widths[i]]);
            left += widths[i];
        }
        return spans;
    }

    // A number with `decimals` decimals and the language's decimal point.
    function decimal(value as Numeric, decimals as Number) as String {
        var text = value.toDouble().format("%." + decimals + "f");
        var point = str(Rez.Strings.DecimalPoint);
        if (point.equals(".")) {
            return text;
        }
        var at = text.find(".");
        return at == null ? text : text.substring(0, at) + point + text.substring(at + 1, text.length());
    }

    // A wind speed: whole numbers without decimals.
    function speed(value as Numeric) as String {
        var rounded = Math.round(value.toDouble() * 10.0d) / 10.0d;
        return rounded == Math.round(rounded) ? rounded.toNumber().toString() : decimal(rounded, 1);
    }

    function rangeUnit(unit as Number) as String {
        return str(unit == Units.RANGE_YD ? Rez.Strings.UnitYd : Rez.Strings.UnitM);
    }

    function windUnit(unit as Number) as String {
        if (unit == Units.WIND_KMH) {
            return str(Rez.Strings.UnitKmh);
        }
        return str(unit == Units.WIND_MPH ? Rez.Strings.UnitMph : Rez.Strings.UnitMps);
    }

    function lengthUnit(unit as Number) as String {
        return str(unit == Units.LENGTH_IN ? Rez.Strings.UnitIn : Rez.Strings.UnitCm);
    }

    function temperatureUnit(unit as Number) as String {
        return str(unit == Units.TEMPERATURE_F ? Rez.Strings.UnitF : Rez.Strings.UnitC);
    }

    // A pressure from hPa, in the unit, with its unit.
    function pressure(hpa as Numeric, unit as Number) as String {
        var value = hpa.toDouble() / Units.hpaPer(unit);
        if (unit == Units.PRESSURE_INHG) {
            return decimal(value, 2) + " " + str(Rez.Strings.UnitInHg);
        }
        return Math.round(value).toNumber() + " " + str(unit == Units.PRESSURE_MMHG ? Rez.Strings.UnitMmHg : Rez.Strings.UnitHpa);
    }

    // A small triangle `gap` above (x, y) pointing up and one as far below
    // pointing down: what UP and DOWN change.
    function arrows(dc as Graphics.Dc, x as Numeric, y as Numeric, gap as Numeric, size as Numeric,
            color as Graphics.ColorType) as Void {
        dc.setColor(color, Graphics.COLOR_TRANSPARENT);
        var top = y - gap;
        var bottom = y + gap;
        dc.fillPolygon([[x - size, top + size / 2], [x + size, top + size / 2], [x, top - size / 2]] as Array<Graphics.Point2D>);
        dc.fillPolygon([[x - size, bottom - size / 2], [x + size, bottom - size / 2], [x, bottom + size / 2]] as Array<Graphics.Point2D>);
    }

    function clickUnit(unit as Number) as String {
        return str(unit == Units.CLICK_MOA ? Rez.Strings.UnitMoa : Rez.Strings.UnitMil);
    }
}
