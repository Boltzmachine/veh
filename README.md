<div align="center">
<h1>Veh</h1>
</div>

**Veh** is a fast, light-weight, and cross-platform image viewer. It is mainly aimed at command line users. The name  **veh** borrows from the famous image viewer [feh](https://github.com/derf/feh). **veh** is written in [vello](https://github.com/linebender/vello), an experimental 2D graphics rendering engine written in Rust, with a focus on GPU compute.



# Installation
**Prerequisite**: You need to have [Rust](https://www.rust-lang.org/tools/install) installed.

To install **veh** for a user
```shell
cargo install --git https://github.com/Boltzmachine/veh.git
```
It will install **veh** to `~/.cargo/bin` by default.

Or system-wide
```shell
sudo -E cargo install --git https://github.com/Boltzmachine/veh.git --root /usr/local  # or other directory
```

# Usage
```shell
veh <image_path>
```

For example,
```shell
veh assets/test.png
```
which will give you the result:

<img src=./assets/screenshot.png style="zoom: 20%" />

Now you can drag the image around by press the *left button* of your mouse. *Middle wheel* for zoom in or out.