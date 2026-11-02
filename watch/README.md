# MX Shooting Calculator on a Garmin watch

A Connect IQ watch app, in Monkey C, for the epix (Gen 2) first. It shows
the clicks to dial and the centimetres at the target for a range, a wind and
the weather. It doesn't solve the flight itself: the phone or desktop app
works out a profile for every rifle and load and sends them, encrypted,
through the app's sync server (see [docs/watch.md](../docs/watch.md)).

On the watch:

- **UP** and **DOWN** change the range, the wind speed, the wind's clock or
  the temperature (and the air pressure, when the barometer gives none);
  **START** moves between them, and a tap picks one. A tap on the rifle's
  name at the top lists the others.
- **MENU** (hold UP): **Rifle** chooses the rifle and load. **Pair with
  phone** shows an 8-digit code: in the app, choose **Garmin watch** and type
  it. The watch then lists the rifles and asks **Use these rifles?**: check
  that they are yours. After that the watch picks up changes by itself when
  it opens, or on **Update from phone**. **About this rifle** shows when it
  was sent, the ethical limit in this weather, how far the table goes and
  where the pressure comes from. **Forget the phone** stops the updates; the
  rifles stay.
- **BACK** leaves the app.

The watch keeps the rifles, its own key pair and the paired app's public
key, and the last range, wind and weather set, and nothing else. It reads
the air pressure from its barometer. It talks only to the sync server,
through Garmin Connect on the phone, and only when it opens while paired, on
**Update from phone** and while pairing.

## SDK and devices

1. Install Java (the compiler needs it): `brew install openjdk@17`.
2. Install the [Connect IQ SDK Manager](https://developer.garmin.com/connect-iq/sdk/),
   sign in with a Garmin account, and download an SDK (9.3.0 or later) and
   the **epix (Gen 2)** device. Without the device the compiler stops with
   "Invalid device id".
3. Make a developer key once, if you have none: the SDK Manager or the
   Monkey C extension for VS Code can, or
   `openssl genrsa -out key.pem 4096 && openssl pkcs8 -topk8 -inform PEM -outform DER -in key.pem -out developer_key.der -nocrypt`.
   `build.sh` looks for it at
   `~/Library/Application Support/Garmin/ConnectIQ/developer_key.der`. Keep
   it out of the repo.

## Build

```sh
./build.sh          # bin/mxsc-watch.prg
./build.sh run      # build, then run it in the simulator
./build.sh test     # the unit tests, in the simulator
```

`DEVICE=fenix7 ./build.sh` builds for another downloaded device (it must
also be listed in `manifest.xml`). The server address is in
`source/Config.mc`.

The unit tests (`source/Tests.mc`) open the app's test vector
(`source/TestVector.mc`, made by the app's tests; don't edit it, rewrite it
with `MXSC_UPDATE_DOCS=1 cargo test -p mxsc watch`)
and check the keys, the box, the profile, the holds and the ethical limits
against the app's.

## Simulator

`./build.sh run` starts the simulator (`ConnectIQ.app` in the SDK) if it
isn't running. Its web requests go out through the computer, so **Pair with
phone** works there too against the real sync server. A local server
(`npx wrangler dev`) speaks plain HTTP, which the simulator refuses while
its **Settings > Use Device HTTPS Requirements** is on; turn it off to try
one, with `source/Config.mc` pointed at it (and don't commit that).

## On the watch

Sideload the `.prg` over USB:

1. Plug the watch in. On a Mac it shows up as an MTP device, so use
   [OpenMTP](https://openmtp.ganeshrvel.com/) to see its files.
2. Copy `bin/mxsc-watch.prg` into `GARMIN/APPS` on the watch.
3. Unplug it. The app is in the watch's app list as **MX Shooting**.

To remove it, delete the file again, or remove it from the watch's app
list.
