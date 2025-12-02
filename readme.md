# Korvatunturi's Box
This is a temporary filesharing server I created for me to use.
It's simple, fast and easy to modify.

# Why yet another filesharing solution?
I wanted to host a simple software for filesharing which was somewhat similar to catbox and other temporary filesharing softwares.

I decided to write the software myself as this seemed like a relatively easy task. This is easier than selfhosting and trying to understand a huge "it just works" monolith system.

No guarantees about this working in the future and / or having backwards compatability. 

# Why no native SSL support despite using actix?
Actix-web HTTP2.0 performance is not worth the hassle, especially for file uploads. It's somewhere around 300x slower. I adopted to using nginx to terminate the HTTP2.0 connection and using a http1.0 connection between nginx <-> korvatunturi-box to talk to actix.

# Configuration?
There's a config.toml file that might or might not be up to date. If it's not, please check `src/settings.rs` for the settings you can tweak without poking at the source.
At any rate, the source code is modular and should be easy to pick up. There is as little interconnectivity as possible (to a reasonable extent)
