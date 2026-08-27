local rows = {
  { key = "enabled", type = "toggle", label = "Enabled", default = true },
  { key = "mode", type = "choice", label = "Mode", default = "safe",
    choices = { { "Safe", "safe" }, { "Fast", "fast" } } },
  { key = "rate", type = "number", label = "Rate", default = 5,
    min = 0, max = 10, step = 2,
    visible_if = { key = "mode", equals = "fast" } },
  { key = "name", type = "text", label = "Name", default = "",
    maxLen = 12, visible_if = { key = "enabled", not_equals = false } },
  { key = "ignored", type = "slider", label = "Unknown row type" },
  { key = "", type = "toggle" },
  "not a row",
}
for i = 1, 3 do
  rows[#rows + 1] = { key = "extra" .. i, type = "toggle", default = false }
end
return rows
