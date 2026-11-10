"""Read-only protocol/certificate probe for the running loopback soak fixture."""
import csv
import hashlib
import json
import socket
import ssl
import sys
import time
import urllib.request


def get(host):
    request = urllib.request.Request('http://127.0.0.1:8080/protocol', headers={'Host': host})
    try:
        with urllib.request.urlopen(request, timeout=5) as response:
            return response.read(128).decode().strip()
    except Exception as exc:
        return 'ERROR:' + type(exc).__name__


def main():
    values = [int(time.time()), get('h2c.soak.test'), get('h2tls.soak.test')]
    try:
        context = ssl.create_default_context()
        context.check_hostname = False
        context.verify_mode = ssl.CERT_NONE  # disposable fixture certificate
        with socket.create_connection(('127.0.0.1', 8443), timeout=5) as raw:
            with context.wrap_socket(raw, server_hostname='soak.test') as tls:
                fingerprint = hashlib.sha256(tls.getpeercert(binary_form=True)).hexdigest()
    except Exception as exc:
        fingerprint = 'ERROR:' + type(exc).__name__
    try:
        with urllib.request.urlopen('http://127.0.0.1:9180/status', timeout=5) as response:
            reload_ok = json.load(response)['config']['last_reload']['ok']
    except Exception as exc:
        reload_ok = 'ERROR:' + type(exc).__name__
    csv.writer(sys.stdout).writerow(values + [fingerprint, reload_ok])


if __name__ == '__main__':
    main()
