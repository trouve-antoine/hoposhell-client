# Buffer limits

`HOPOSHELL_MAX_BUFFER_MB` sets the client's buffer limit as a positive whole
number of mebibytes (1 MiB = 1,048,576 bytes). The default is **64**. Zero,
negative, fractional, and overflowing values are rejected at startup.

```sh
HOPOSHELL_MAX_BUFFER_MB=32 hopo connect shell_YmXKOXupM9
```

The client prints an error and exits with status **1** when a buffer limit is
exceeded, including from a worker thread. A supervisor or retry wrapper can
then restart it.

The limit applies to incoming protocol buffers, output history, pending command
chunks, response chunks, listings, and buffered file, HTTP, TCP, script, and
decompressed outputs. Both message queues share one aggregate queue budget,
so many small messages cannot grow the queues without bound. Pending commands
also have an aggregate limit, including message metadata. Reads and writes are
checked before appending more data. The zstd decoder's history window is bounded
as well.

This is **not a process-wide memory limit**: buffers can coexist, serialization
and parsing can make copies, and libraries and child processes allocate their
own memory. Child scripts' stderr is discarded instead of being retained in
memory (it was previously captured but unused). Long-running streamed shell
output is allowed as long as the queue and history limits are not exceeded.

Exiting the client does not guarantee that descendants exit. Use OS-level
supervision for process cleanup and, on Linux, a hard aggregate memory limit.
