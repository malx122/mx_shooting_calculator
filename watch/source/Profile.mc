// A watch profile (docs/watch.md, "The watch profile (format 2)") and the
// watch's sums. It must agree with crates/app/src/watch/profile.rs.
import Toybox.Lang;
import Toybox.Math;
import Toybox.StringUtil;

module Units {
    const CLICK_MIL = 0;
    const CLICK_MOA = 1;
    const RANGE_M = 0;
    const RANGE_YD = 1;
    const WIND_MPS = 0;
    const WIND_KMH = 1;
    const WIND_MPH = 2;
    const LENGTH_CM = 0;
    const LENGTH_IN = 1;
    const TEMPERATURE_C = 0;
    const TEMPERATURE_F = 1;
    const PRESSURE_HPA = 0;
    const PRESSURE_MMHG = 1;
    const PRESSURE_INHG = 2;

    const METRES_PER_YARD = 0.9144d;
    const CM_PER_INCH = 2.54d;
    // 1 MOA in milliradians: π/10800 rad.
    const MRAD_PER_MOA = 1000.0d * Math.PI / 10800.0d;

    // Metres per one of the range unit.
    function metresPer(rangeUnit as Number) as Double {
        return rangeUnit == RANGE_YD ? METRES_PER_YARD : 1.0d;
    }

    // Metres per second per one of the wind unit.
    function mpsPer(windUnit as Number) as Double {
        if (windUnit == WIND_KMH) {
            return 1.0d / 3.6d;
        }
        if (windUnit == WIND_MPH) {
            return 0.44704d;
        }
        return 1.0d;
    }

    // hPa per one of the pressure unit (as the phone app counts them).
    function hpaPer(pressureUnit as Number) as Double {
        if (pressureUnit == PRESSURE_MMHG) {
            return 1.0d / 0.750062d;
        }
        if (pressureUnit == PRESSURE_INHG) {
            return 1.0d / 0.02953d;
        }
        return 1.0d;
    }

    // A temperature in the unit, from °C.
    function fromCelsius(celsius as Numeric, unit as Number) as Double {
        return unit == TEMPERATURE_F ? celsius.toDouble() * 1.8d + 32.0d : celsius.toDouble();
    }

    // A temperature in °C, from the unit.
    function toCelsius(value as Numeric, unit as Number) as Double {
        return unit == TEMPERATURE_F ? (value.toDouble() - 32.0d) / 1.8d : value.toDouble();
    }
}

// What to dial for one shot.
class Hold {
    // Up and right, milliradians (negative: down, left).
    var elevationMrad as Double;
    var windageMrad as Double;

    function initialize(elevation as Double, windage as Double) {
        elevationMrad = elevation;
        windageMrad = windage;
    }
}

// The profile's layout.
module ProfileFormat {
    const VERSION = 2;
    const MIN_ROWS = 4;
    const MAX_ROWS = 120;
    const MAX_NAME = 64;
    const MAX_GRID = 8;
    const MAX_COUNT = 16;
    const COLUMNS = 8;
    const ID_LEN = 8;
    // Where the grid's temperatures start.
    const AT_GRID = 29;
}

class Profile {
    // The bundle it came in: its version (Unix seconds when it was made),
    // and its place in it.
    var version as Number = 0;
    var index as Number = 0;
    var count as Number = 1;
    // Hex: which rifle and load it is, across updates.
    var id as String = "";
    var name as String = "";
    var clickUnit as Number = 0;
    // In the click unit.
    var clickValue as Double = 0.1d;
    var rangeUnit as Number = 0;
    var windUnit as Number = 0;
    var lengthUnit as Number = 0;
    var temperatureUnit as Number = 0;
    var pressureUnit as Number = 0;
    // Metres between rows; row i is at (i + 1) × step.
    var step as Number = 1;
    var rows as Number = 0;
    // The grid: °C and hPa, each rising.
    var temperatures as Array<Double> = [] as Array<Double>;
    var pressures as Array<Double> = [] as Array<Double>;

    private var _scales as Array<Double> = [] as Array<Double>;
    // Per cell, metres; all 0: no ethical limit.
    private var _ethical as Array<Number> = [] as Array<Number>;
    private var _bytes as ByteArray;
    private var _valuesAt as Number = 0;
    private var _options as { :offset as Number, :endianness as Endian };

    // The profile in `bytes`, or null if they aren't a whole, sane format 2
    // profile.
    static function parse(bytes as ByteArray) as Profile or Null {
        try {
            return parseOrThrow(bytes);
        } catch (e) {
            return null;
        }
    }

    private static function parseOrThrow(bytes as ByteArray) as Profile or Null {
        var profile = new Profile(bytes);
        return profile.read() ? profile : null;
    }

    // Reads the bytes into the fields; false if they aren't a profile.
    function read() as Boolean {
        var bytes = _bytes;
        var size = bytes.size();
        if (size < ProfileFormat.AT_GRID + 1 || bytes[0] != ProfileFormat.VERSION) {
            return false;
        }
        version = number(Lang.NUMBER_FORMAT_UINT32, 1);
        index = bytes[5];
        count = bytes[6];
        id = Seal.toHex(bytes.slice(7, 7 + ProfileFormat.ID_LEN));
        clickUnit = bytes[15];
        clickValue = float(16);
        rangeUnit = bytes[20];
        windUnit = bytes[21];
        lengthUnit = bytes[22];
        temperatureUnit = bytes[23];
        pressureUnit = bytes[24];
        step = number(Lang.NUMBER_FORMAT_UINT16, 25);
        rows = number(Lang.NUMBER_FORMAT_UINT16, 27);
        if (clickUnit > Units.CLICK_MOA || rangeUnit > Units.RANGE_YD
                || windUnit > Units.WIND_MPH || lengthUnit > Units.LENGTH_IN
                || temperatureUnit > Units.TEMPERATURE_F || pressureUnit > Units.PRESSURE_INHG
                || !(clickValue >= 0.001d) || clickValue > 100.0d
                || step < 1 || rows < ProfileFormat.MIN_ROWS || rows > ProfileFormat.MAX_ROWS
                || count < 1 || count > ProfileFormat.MAX_COUNT || index >= count) {
            return false;
        }
        var at = ProfileFormat.AT_GRID;
        var nt = bytes[at];
        at += 1;
        if (nt < 2 || nt > ProfileFormat.MAX_GRID || size < at + nt + 1) {
            return false;
        }
        for (var i = 0; i < nt; i++) {
            temperatures.add(number(Lang.NUMBER_FORMAT_SINT8, at + i).toDouble());
        }
        at += nt;
        var np = bytes[at];
        at += 1;
        if (np < 2 || np > ProfileFormat.MAX_GRID || size < at + 2 * np + 4 * ProfileFormat.COLUMNS + 2 * nt * np + 1) {
            return false;
        }
        for (var i = 0; i < np; i++) {
            pressures.add(number(Lang.NUMBER_FORMAT_UINT16, at + 2 * i).toDouble());
        }
        at += 2 * np;
        if (!rising(temperatures) || !rising(pressures)) {
            return false;
        }
        for (var column = 0; column < ProfileFormat.COLUMNS; column++) {
            var scale = float(at + 4 * column);
            if (!(scale > 0.0d) || scale > 1000.0d) {
                return false;
            }
            _scales.add(scale);
        }
        at += 4 * ProfileFormat.COLUMNS;
        for (var cell = 0; cell < nt * np; cell++) {
            _ethical.add(number(Lang.NUMBER_FORMAT_UINT16, at + 2 * cell));
        }
        at += 2 * nt * np;
        var nameLength = bytes[at];
        at += 1;
        if (nameLength > ProfileFormat.MAX_NAME
                || size != at + nameLength + 2 * ProfileFormat.COLUMNS * rows * nt * np) {
            return false;
        }
        if (nameLength > 0) {
            var text = StringUtil.convertEncodedString(bytes.slice(at, at + nameLength), {
                :fromRepresentation => StringUtil.REPRESENTATION_BYTE_ARRAY,
                :toRepresentation => StringUtil.REPRESENTATION_STRING_PLAIN_TEXT,
                :encoding => StringUtil.CHAR_ENCODING_UTF8
            });
            // Anything but text would break every screen that shows it.
            if (!(text instanceof String)) {
                return false;
            }
            name = text;
        }
        _valuesAt = at + nameLength;
        return true;
    }

    // Each number above the one before.
    private static function rising(values as Array<Double>) as Boolean {
        for (var i = 1; i < values.size(); i++) {
            if (!(values[i] > values[i - 1])) {
                return false;
            }
        }
        return true;
    }

    function initialize(bytes as ByteArray) {
        _bytes = bytes;
        _options = { :offset => 0, :endianness => Lang.ENDIAN_LITTLE };
    }

    // A little-endian number at `offset`. One options dictionary serves
    // every read, so reading a profile doesn't make a dictionary per number.
    function decode(format as NumberFormat, offset as Number) as Numeric {
        _options[:offset] = offset;
        return _bytes.decodeNumber(format, _options);
    }

    function float(offset as Number) as Double {
        return (decode(Lang.NUMBER_FORMAT_FLOAT, offset) as Float).toDouble();
    }

    function number(format as NumberFormat, offset as Number) as Number {
        return decode(format, offset).toNumber();
    }

    // The bytes the profile was read from, to keep.
    function bytes() as ByteArray {
        return _bytes;
    }

    // Column `column` of row `row` in cell (t, p), mrad.
    function value(t as Number, p as Number, row as Number, column as Number) as Double {
        var cell = t * pressures.size() + p;
        var offset = _valuesAt + 2 * ((cell * rows + row) * ProfileFormat.COLUMNS + column);
        return decode(Lang.NUMBER_FORMAT_SINT16, offset).toDouble() * _scales[column];
    }

    // The farthest range, metres.
    function lastM() as Number {
        return rows * step;
    }

    // Whether the weather is past the grid (the sums then hold it at the
    // grid's edge).
    function outsideGrid(temperatureC as Numeric, pressureHpa as Numeric) as Boolean {
        return temperatureC < temperatures[0] || temperatureC > temperatures[temperatures.size() - 1]
            || pressureHpa < pressures[0] || pressureHpa > pressures[pressures.size() - 1];
    }

    // What to dial at `rangeM` metres in a wind of `windMps` from `clock`
    // o'clock, at `temperatureC` and `pressureHpa`; null before the first
    // row or past the last.
    function hold(rangeM as Numeric, windMps as Numeric, clock as Numeric,
            temperatureC as Numeric, pressureHpa as Numeric) as Hold or Null {
        var range = rangeM.toDouble();
        var position = range / step - 1.0d;
        if (position < -1.0e-9d || position > (rows - 1) + 1.0e-9d) {
            return null;
        }
        var first = Math.floor(position).toNumber() - 1;
        if (first > rows - 4) {
            first = rows - 4;
        }
        if (first < 0) {
            first = 0;
        }
        var along = weights([first.toDouble(), first + 1.0d, first + 2.0d, first + 3.0d] as Array<Double>, position);
        var byT = weights(temperatures, clampTo(temperatures, temperatureC.toDouble()));
        var byP = weights(pressures, clampTo(pressures, pressureHpa.toDouble()));
        var sums = new Array<Double>[ProfileFormat.COLUMNS];
        for (var column = 0; column < ProfileFormat.COLUMNS; column++) {
            sums[column] = 0.0d;
        }
        for (var t = 0; t < byT.size(); t++) {
            for (var p = 0; p < byP.size(); p++) {
                var cell = byT[t] * byP[p];
                for (var k = 0; k < 4; k++) {
                    var row = first + k;
                    var weight = cell * along[k] * (row + 1) * step;
                    for (var column = 0; column < ProfileFormat.COLUMNS; column++) {
                        sums[column] = sums[column] + weight * value(t, p, row, column);
                    }
                }
            }
        }
        var angle = clock.toDouble() * Math.PI / 6.0d;
        var cross = windMps.toDouble() * Math.sin(angle);
        var head = windMps.toDouble() * Math.cos(angle);
        return new Hold(
            (sums[0] + sums[2] * cross + sums[4] * head + sums[6] * cross * head) / range,
            (sums[1] + sums[3] * cross + sums[5] * head + sums[7] * cross * head) / range);
    }

    // The ethical limit in this weather, metres (straight lines between the
    // grid's cells); null when there is none.
    function ethicalLimit(temperatureC as Numeric, pressureHpa as Numeric) as Double or Null {
        var any = false;
        for (var i = 0; i < _ethical.size(); i++) {
            any = any || _ethical[i] > 0;
        }
        if (!any) {
            return null;
        }
        var t = bracket(temperatures, temperatureC.toDouble());
        var p = bracket(pressures, pressureHpa.toDouble());
        var below = t[0].toNumber();
        var left = p[0].toNumber();
        var low = between(cellLimit(below, left), cellLimit(below, left + 1), p[1]);
        var high = between(cellLimit(below + 1, left), cellLimit(below + 1, left + 1), p[1]);
        return between(low, high, t[1]);
    }

    private function cellLimit(t as Number, p as Number) as Double {
        return _ethical[t * pressures.size() + p].toDouble();
    }

    private function between(low as Double, high as Double, share as Double) as Double {
        return low + (high - low) * share;
    }

    // An angle in the click unit.
    function inClickUnit(mrad as Double) as Double {
        return clickUnit == Units.CLICK_MOA ? mrad / Units.MRAD_PER_MOA : mrad;
    }

    // Clicks to dial for an angle: round(angle ÷ click value).
    function clicks(mrad as Double) as Number {
        return Math.round(inClickUnit(mrad) / clickValue).toNumber();
    }

    // What an angle comes to at the target, in the short length unit.
    function atTarget(mrad as Double, rangeM as Numeric) as Double {
        var cm = mrad * rangeM.toDouble() / 10.0d;
        return lengthUnit == Units.LENGTH_IN ? cm / Units.CM_PER_INCH : cm;
    }
}

// The weight of each node in the polynomial through all of them, at x
// (Lagrange).
function weights(nodes as Array<Double>, x as Double) as Array<Double> {
    var out = [] as Array<Double>;
    for (var i = 0; i < nodes.size(); i++) {
        var weight = 1.0d;
        for (var j = 0; j < nodes.size(); j++) {
            if (j != i) {
                weight = weight * (x - nodes[j]) / (nodes[i] - nodes[j]);
            }
        }
        out.add(weight);
    }
    return out;
}

// x held within the nodes' span.
function clampTo(nodes as Array<Double>, x as Double) as Double {
    var low = nodes[0];
    var high = nodes[nodes.size() - 1];
    return x < low ? low : x > high ? high : x;
}

// [the node at or below x (never the last), how far on to the next].
function bracket(nodes as Array<Double>, x as Double) as Array<Double> {
    var held = clampTo(nodes, x);
    var below = nodes.size() - 2;
    for (var i = 0; i < nodes.size() - 1; i++) {
        if (held <= nodes[i + 1]) {
            below = i;
            break;
        }
    }
    return [below.toDouble(), (held - nodes[below]) / (nodes[below + 1] - nodes[below])] as Array<Double>;
}
