#!/usr/bin/bash

scp abyss:ws/omnirec/omnirec/src-tauri/target/release/bundle/deb/omnirec\*.deb .

sudo apt remove -y omnirec

sudo apt install -y ./omnirec*.deb

omnirec 2>&1 | tee omnirec.log

scp omnirec.log abyss:ws/omnirec/omnirec

sudo apt remove -y omnirec

