<div align="center">
<h1>Veh</h1>
</div>

**Veh** is a fast, light-weight, and cross-platform image viewer. It is mainly aimed at command line users. The name  **veh** borrows from the famous image viewer [feh](https://github.com/derf/feh). **veh** is written in [vello](https://github.com/linebender/vello), an experimental 2D graphics rendering engine written in Rust, with a focus on GPU compute.


# Installation
Clone the repo and run
```shell
cargo install --path .
```
It will install **veh** to `~/.cargo/bin` by default.

Or system-wide
```shell
sudo -E cargo install --path . --root /usr/local  # or other directory
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

![alt text](./assets/screenshot.png)
