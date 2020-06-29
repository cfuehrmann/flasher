namespace Flasher.Host.Model
{
    public class UpdateCardRequest
    {
#nullable disable warnings
        public string? prompt { get; set; }
        public string? solution { get; set; }
        public bool isMinor { get; set; }
    }
}