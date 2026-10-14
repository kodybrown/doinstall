var app = WebApplication.CreateBuilder(args).Build();
app.MapGet("/health", () => Results.Ok("ready"));
app.MapGet("/api/health", () => Results.Ok("ready"));
app.MapGet("/", () => "doinstall sample");
app.Run();
