cargo_conf_file=$TRAVIS_HOME/.cargo/config
touch $cargo_conf_file
cat >>$cargo_conf_file <<EOL
[target.i686-pc-windows-gnu]
linker = "i686-w64-mingw32-gcc"
ar = "i686-w64-mingw32-ar"

[target.x86_64-pc-windows-gnu]
linker = "x86_64-w64-mingw32-gcc"
ar = "x86_64-w64-mingw32-ar"
EOL