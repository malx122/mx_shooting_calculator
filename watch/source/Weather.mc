// The weather the holds are for: the temperature the hunter sets, and the
// air pressure from the watch's barometer (set by hand when it gives none).
import Toybox.Activity;
import Toybox.Lang;

module Weather {
    // Until the hunter sets one.
    const DEFAULT_C = 10.0d;
    const DEFAULT_HPA = 1013.0d;

    function temperatureC() as Double {
        var kept = Store.number(Store.TEMPERATURE);
        return kept != null ? kept.toDouble() : DEFAULT_C;
    }

    function setTemperatureC(value as Double) as Void {
        Store.save(Store.TEMPERATURE, value.toFloat());
    }

    // The barometer's reading where the watch is, hPa, or null.
    function barometerHpa() as Double or Null {
        var info = Activity.getActivityInfo();
        var pascals = info.ambientPressure;
        if (pascals == null) {
            pascals = info.rawAmbientPressure;
        }
        // Anything outside what air on Earth can be is no reading.
        if (pascals == null || !(pascals > 30000.0) || pascals > 110000.0) {
            return null;
        }
        return pascals.toDouble() / 100.0d;
    }

    // The pressure to use, hPa: the barometer's, else the one set by hand.
    function pressureHpa() as Double {
        var measured = barometerHpa();
        if (measured != null) {
            return measured;
        }
        var kept = Store.number(Store.PRESSURE);
        return kept != null ? kept.toDouble() : DEFAULT_HPA;
    }

    function setPressureHpa(value as Double) as Void {
        Store.save(Store.PRESSURE, value.toFloat());
    }
}
