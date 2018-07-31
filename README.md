## hack

```
cargo run
```

## install

```
cargo build --release
cp target/release/batterybar /usr/local/bin
cp com.github.blandinw.batterybar.plist ~/Library/LaunchAgents
launchctl load ~/Library/LaunchAgents/com.github.blandinw.batterybar.plist
```