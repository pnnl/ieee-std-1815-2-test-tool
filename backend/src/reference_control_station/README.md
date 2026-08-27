# reference_control_station

DNP3 Reference Control Station application. Connects to an outstation over TCP/IP, polls data, sends controls.

## Running

```sh
cargo run -- --outstation-ip 127.0.0.1 --outstation-port 20000
```

CLI options:
- `--outstation-ip <IP>` — outstation IP address (default: `127.0.0.1`)
- `--outstation-port <PORT>` — outstation TCP port (default: `20000`)
- `--control-station-address <ADDR>` — DNP3 address of this control station / master (default: `1`)
- `--outstation-address <ADDR>` — DNP3 address of the remote outstation (default: `1024`)
- `--profile <PATH>` — PICS profile JSON path (optional; enables enrichment and curve/schedule databases)
- `--log-level <LEVEL>` — log level: error, warn, info, debug, trace (default: `info`)

## Diagnosing connectivity issues

When connecting to a real outstation, work through these steps in order.

**1. Basic network reachability**
```sh
ping <outstation-ip>
traceroute <outstation-ip>   # shows where packets are dropped if ping fails
```

**2. TCP port connectivity**
```sh
nc -zv <outstation-ip> <outstation-port>
# or
telnet <outstation-ip> <outstation-port>
```
If this fails but ping succeeds, the outstation process is not running or a firewall is blocking the port.

**3. Check firewall rules**

On Linux:
```sh
sudo iptables -L
# or
sudo ufw status
```
On Windows, check Windows Defender Firewall for the relevant port. The outstation host may also be blocking inbound connections.

**4. Verify the outstation is listening**

Run this on the outstation machine:
```sh
ss -tlnp | grep <port>
# or
netstat -tlnp | grep <port>
```

**5. Watch live traffic with tcpdump**
```sh
sudo tcpdump -i any host <outstation-ip> and port <outstation-port>
```
A completed TCP three-way handshake followed by no further traffic typically indicates a DNP3 address mismatch — see step 6.

**6. Check DNP3 addresses**

The most common cause of a connected-but-silent session is mismatched DNP3 link-layer addresses. Confirm that `--outstation-address` matches the address configured on the real outstation, and that `--control-station-address` matches what the outstation expects from its master. Use Wireshark with the DNP3 dissector to inspect the link-layer source/destination fields in captured frames.

**7. Read the control station logs**

The `ClientStateListener` emits structured log lines:
- `TCP: connecting to outstation...` — TCP connect in progress
- `TCP: connected to outstation` — TCP layer is up; DNP3 startup sequence begins
- `TCP: connection failed, retrying in Xs` — TCP never completed; check steps 1–4
- `TCP: disconnected, reconnecting in Xs` — connection dropped after establishment

If the log shows `TCP: connected` but no data is exchanged, the issue is at the DNP3 layer (addresses, unsolicited configuration, or integrity poll rejection).
