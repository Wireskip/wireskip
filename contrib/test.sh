#!/bin/sh
set -ex

export RUST_LOG=debug
export RUST_BACKTRACE=full

cleanup() { kill -TERM $pid1 $pid2 $pid3 $pid4 $pid5 $pid6 $pid7 >/dev/null 2>&1; }

trap cleanup INT TERM EXIT

# relay 1
./target/debug/relay host 127.0.0.1:7777 >relay1.log 2>&1 & pid1=$!

# relay 2
./target/debug/relay host 127.0.0.1:8888 >relay2.log 2>&1 & pid2=$!

# relay 3
./target/debug/relay host 127.0.0.1:9999 >relay3.log 2>&1 & pid3=$!

sleep 1

# client / socks
./target/debug/relay join \
    127.0.0.1:7777 127.0.0.1:8888 127.0.0.1:9999 \
    >client.log 2>&1 \
    & pid4=$!

tail -n0 -f relay?.log client.log & pid5=$!

sleep 1

# test TCP
curl -vvvvv --http2 -sx socks5h://localhost:1080 https://ifconfig.co/json

# test UDP
nc -k -v -l -u 127.0.0.1 12345 >nc_s.log 2>&1 & pid6=$!

printf '\x00\x00\x00\x01\x7F\x00\x00\x01\x30\x39hello, world!' \
    | nc -u 127.0.0.1 1081

if [ -n "$WAIT" ]; then
    read -n1 -p '?> '
else
    sleep 1
fi

cleanup
echo 'all done'
