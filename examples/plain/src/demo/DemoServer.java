package demo;

import java.io.OutputStream;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.nio.charset.StandardCharsets;

final class DemoServer {
    static void serve(String name, String[] args) throws Exception {
        int port = args.length == 0 ? 0 : Integer.parseInt(args[0]);
        try (ServerSocket server = new ServerSocket(port, 16, InetAddress.getByName("127.0.0.1"))) {
            System.out.println("READY " + name + " " + server.getLocalPort());
            System.out.flush();
            while (true) {
                try (Socket socket = server.accept(); OutputStream out = socket.getOutputStream()) {
                    byte[] body = (name + "\n").getBytes(StandardCharsets.UTF_8);
                    out.write(("HTTP/1.1 200 OK\r\nContent-Length: " + body.length + "\r\nConnection: close\r\n\r\n").getBytes(StandardCharsets.UTF_8));
                    out.write(body);
                }
            }
        }
    }
}
