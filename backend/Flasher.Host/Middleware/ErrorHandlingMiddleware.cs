using System.Net;
using Microsoft.AspNetCore.Http.Extensions;
using Microsoft.AspNetCore.Mvc;

namespace Flasher.Host.Middleware;

public class ErrorHandlingMiddleware(RequestDelegate next, ILogger<ErrorHandlingMiddleware> logger)
{
    private const int InternalServerError = (int)HttpStatusCode.InternalServerError;

    public async Task InvokeAsync(HttpContext context)
    {
        try
        {
            await next(context);
        }
        catch (Exception ex)
        {
            logger.LogError(ex, "An unhandled exception occurred");
            await HandleExceptionAsync(context, ex);
        }
    }

    private static async Task HandleExceptionAsync(HttpContext context, Exception exception)
    {
        context.Response.StatusCode = InternalServerError;

        var problemDetails = new ProblemDetails
        {
            Type = "https://tools.ietf.org/html/rfc9110#section-15.6.1",
            Title = "An unexpected error occurred",
            Status = InternalServerError,
            Detail = "Internal Server Error",
            Instance = context.Request.GetEncodedPathAndQuery(),
        };

        await context.Response.WriteAsJsonAsync(
            problemDetails,
            System.Text.Json.JsonSerializerOptions.Web,
            "application/problem+json"
        );
    }
}
