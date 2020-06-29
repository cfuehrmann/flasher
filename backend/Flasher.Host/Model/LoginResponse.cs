using Flasher.Store.AutoSaving;

namespace Flasher.Host.Model
{
    public class LoginResponse
    {
        public LoginResponse(string jsonWebToken)
        {
            this.jsonWebToken = jsonWebToken;
        }

        public string jsonWebToken { get; }
        public AutoSave? autoSave { get; set; }
    }
}