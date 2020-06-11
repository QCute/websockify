# websockify 
A Client Configurable WebSockets(Security) to TCP Proxy Server

## Usage

```
websockify 1.0.0

USAGE:
    websockify [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -d, --dst <dst>                                Destination Address or Unix Domain Socket Path [default: 127.0.0.1]
    -l, --listen <listen>                          Listen Address [default: 0.0.0.0]
    -m, --port-map-type <port-map-type>            Proxy Type [default: prefix]  [possible values: Domain, Prefix]
    -x, --port-restrict-max <port-restrict-max>    Proxy Port Restrict Max [min, max) [default: 0]
    -i, --port-restrict-min <port-restrict-min>    Proxy Port Restrict Min [min, max) [default: 0]
    -c, --ssl-cert <ssl-cert>                      SSL Cert File Path [default: ]
    -k, --ssl-key <ssl-key>                        SSL Key File Path [default: ]
    -s, --ssl-port <ssl-port>                      SSL Port [default: 0]
    -t, --tcp-port <tcp-port>                      TCP Port [default: 0]
    -o, --timeout <timeout>                        Client Request Timeout [default: 10]
```

# Network Mode

## proxy
```
websockify --ssl-port=8974 --port-map-type=domain --port-restrict-min=10000 --port-restrict-max=20000 --ssl-cert=example.com.crt --ssl-key=example.com.key
```

## server
```
nc -lk 10086
```

## client (js)
```
let ws = new WebSocket("wss://10086.example.com:8974/");
ws.onopen = (event) => ws.send("hey, I'm web socket");
```

# Unix Domain Socket Mode

## proxy
```
websockify --tcp-port=8998 --dst=/tmp/websockify/
```

## server
```
nc -Ulk /tmp/websockify/10010.sock
```

## client (js)
```
let ws = new WebSocket("wss://example.com:8998/10010.sock/");
ws.onopen = (event) => ws.send("hey, I'm web socket");
```
