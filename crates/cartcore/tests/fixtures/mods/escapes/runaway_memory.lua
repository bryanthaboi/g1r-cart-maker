local blob = string.rep("x", 512 * 1024 * 1024)
return { { key = "blob", type = "text", default = blob } }
