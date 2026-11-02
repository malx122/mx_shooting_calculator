// The one place the watch learns where the app's sync server is.
import Toybox.Lang;

module Config {
    // The sync server (docs/watch.md, "Server API"). No other service is
    // ever asked for anything.
    const SERVER = "https://mxsc-sync.malx122.workers.dev";
}
