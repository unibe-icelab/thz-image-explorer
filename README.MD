# Explorer Software for the Data of THz time-domain imaging analysis

<a href="https://github.com/hacknus/thz-image-explorer/releases"><img src="icons/icon.png" alt=“” width="100" height="100"> </img> </a>

This Application is written in Rust and the GUI is based on egui and bevy.

![Screenshot of the application on macOS](paper/screenshot.png)

A sample scan of a THz time-domain 2D imaging system can be found in the `sample_data` directory.

## Installation:

### Download pre-built executables

[Binary bundles](https://github.com/hacknus/thz-image-explorer/releases) are available for Linux, macOS and Windows.

Running the apple silicon (thz-image-explorer-aarch64-apple-darwin.app) binary may result to the message "THz Image
Explorer is damaged and cannot be opened.", to get
around this you first need to run:
`xattr -rd com.apple.quarantine THz\ Image\ Explorer.app`

On Linux first install the following:

```sh
sudo apt-get install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev libasound2-dev
```

### Compile from source

The source code can be run using `cargo run` or bundled to a platform-executable using cargo bundle.
Currently [cargo bundle](https://github.com/burtonageo/cargo-bundle) only supports linux and macOS
bundles [see github issue](https://github.com/burtonageo/cargo-bundle/issues/77).
As a work-around we can use [cargo wix](https://github.com/volks73/cargo-wix) to create a windows installer.

On Linux first install the following:

```sh
sudo apt-get install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev libasound2-dev
```

After installing
`cargo install cargo-bundle` on linux and macOS or `cargo install cargo-wix` on windows
run
`cargo bundle` on linux and macOS or `cargo wix` on windows to create platform-executable bundles.

On macOS the `DotTHzQLExtension.appex` Plugin will automatically be installed in the
`THz Image Explorer.app/Contents/PlugIns` directory. The source code of the plugin can be
found [here](https://github.com/hacknus/DotTHzQL). Note: This plugin requires HDF5 to be installed system-wide.

It can be compiled and run on all platforms.

## PSF file generation:

You need to performa knife-edge scan and create a `.thz` file with all entries. An example can be found in the `sample_data/example_beam_width/` directories.
Then, use the following command (replace the paths with your own):

```shell
python generate_psf.py --path_x sample_data/example_beam_width/measurement_x/data/1750085285.8557956_data.thz --path_y sample_data/example_beam_width/measurement_y/data/1750163177.929295_data.thz
```

to generate a `psf.npz` file that can be loaded in the settings of THz-image-explorer to remove the PSF blur.

Tested on:

- MacOS 12.4 Monterey x86
- MacOS 10.14.6 Mojave x86
- Debian 12 (Testing) x86
- Windows 10 x86
