import socket

# 创建socket对象
client = socket.socket()

# 连接到服务器的IP和端口
client.connect(('127.0.0.1', 25565))

# 发送数据到服务器
client.send("你好，我是客户端。".encode())

# 关闭连接
client.close()