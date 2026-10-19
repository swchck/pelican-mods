using System;
using System.Collections.Concurrent;
using System.IO;
using System.Net.WebSockets;
using System.Text;
using System.Threading;
using System.Threading.Channels;
using System.Threading.Tasks;
using Newtonsoft.Json;

namespace PelicanCompanion;

/// <summary>The WebSocket link to the launcher, kept alive on a background thread.</summary>
/// <remarks>
/// Commands are only queued here; the game thread drains <see cref="Inbox"/> because game
/// state must not be touched from anywhere else.
/// </remarks>
internal sealed class Bridge : IDisposable
{
    private readonly Uri Url;
    private readonly Action<string> Log;
    private readonly Channel<string> Outbox = Channel.CreateUnbounded<string>();
    private readonly CancellationTokenSource Stop = new();

    public ConcurrentQueue<Command> Inbox { get; } = new();

    /// <summary>Raised on the bridge thread after each (re)connect, before anything else is sent.</summary>
    public event Action? Connected;

    private volatile bool Online;

    public Bridge(Uri url, Action<string> log)
    {
        this.Url = url;
        this.Log = log;
    }

    /// <summary>Starts connecting; subscribe to <see cref="Connected"/> first.</summary>
    public void Start() => Task.Run(this.RunAsync);

    /// <summary>Queues a message; dropped while offline, since a reconnect resends everything.</summary>
    public void Send(object message)
    {
        if (this.Online)
            this.Outbox.Writer.TryWrite(Protocol.Write(message));
    }

    public void Dispose()
    {
        this.Stop.Cancel();
        this.Outbox.Writer.TryComplete();
    }

    private async Task RunAsync()
    {
        var delay = TimeSpan.FromSeconds(1);
        while (!this.Stop.IsCancellationRequested)
        {
            try
            {
                using var socket = new ClientWebSocket();
                await socket.ConnectAsync(this.Url, this.Stop.Token);
                var connectedAt = DateTime.UtcNow;
                // anything queued while offline describes a state that's gone; the
                // Connected handlers send a fresh picture instead
                while (this.Outbox.Reader.TryRead(out _)) { }
                this.Online = true;
                this.Connected?.Invoke();
                using var linked = CancellationTokenSource.CreateLinkedTokenSource(this.Stop.Token);
                Task reading = this.ReadAsync(socket, linked.Token);
                Task writing = this.WriteAsync(socket, linked.Token);
                await Task.WhenAny(reading, writing);
                linked.Cancel();
                this.Online = false;
                // only a link that held up earns a quick retry; one dropped at once would
                // otherwise resend the whole picture from the game thread every second
                if (DateTime.UtcNow - connectedAt > TimeSpan.FromSeconds(10))
                    delay = TimeSpan.FromSeconds(1);
            }
            catch (Exception ex) when (ex is WebSocketException or IOException or OperationCanceledException)
            {
            }
            catch (Exception ex)
            {
                this.Log($"launcher link failed: {ex}");
            }
            if (this.Stop.IsCancellationRequested)
                return;
            // the launcher may have restarted and come back on the same port
            await Task.Delay(delay).ContinueWith(_ => { });
            delay = TimeSpan.FromSeconds(Math.Min(delay.TotalSeconds * 2, 15));
        }
    }

    private async Task ReadAsync(ClientWebSocket socket, CancellationToken token)
    {
        var buffer = new byte[64 * 1024];
        using var message = new MemoryStream();
        while (!token.IsCancellationRequested && socket.State == WebSocketState.Open)
        {
            WebSocketReceiveResult result = await socket.ReceiveAsync(buffer, token);
            if (result.MessageType == WebSocketMessageType.Close)
                return;
            message.Write(buffer, 0, result.Count);
            if (!result.EndOfMessage)
                continue;
            string text = Encoding.UTF8.GetString(message.GetBuffer(), 0, (int)message.Length);
            message.SetLength(0);
            try
            {
                Command? command = JsonConvert.DeserializeObject<Command>(text, Protocol.Json);
                if (command is not null)
                    this.Inbox.Enqueue(command);
            }
            catch (JsonException ex)
            {
                this.Log($"ignored a malformed launcher message: {ex.Message}");
            }
        }
    }

    private async Task WriteAsync(ClientWebSocket socket, CancellationToken token)
    {
        await foreach (string text in this.Outbox.Reader.ReadAllAsync(token))
        {
            await socket.SendAsync(Encoding.UTF8.GetBytes(text), WebSocketMessageType.Text, true, token);
        }
    }
}
