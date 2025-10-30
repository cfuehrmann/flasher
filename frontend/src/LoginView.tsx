import { useState } from "react";

import { ButtonBar, LoginButton } from "./Buttons";

interface Props {
  userName: string;
  password: string;
  onOk: (userName: string, password: string) => Promise<void>;
}

export function LoginView(props: Props) {
  const [credentials, setCredentials] = useState({
    userName: props.userName,
    password: props.password,
  });

  return (
    <div className="w3-card">
      <div className="w3-container w3-dark-blue">
        <h2>Login</h2>
      </div>
      <div className="w3-container">
        <br />
        <label htmlFor="username">User Name</label>
        <input
          id="username"
          className="w3-input"
          type="text"
          autoComplete="username"
          onChange={(event) => setUserName(event.target.value)}
          value={credentials.userName}
        />
        <br />
        <label htmlFor="password">Password</label>
        <input
          id="password"
          className="w3-input"
          type="password"
          autoComplete="current-password"
          onChange={(event) => setPassword(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              void props.onOk(credentials.userName, credentials.password);
            }
          }}
          value={credentials.password}
        />
        <br />
        <ButtonBar>
          <LoginButton
            width="26%"
            onClick={() =>
              void props.onOk(credentials.userName, credentials.password)
            }
          />
        </ButtonBar>
      </div>
      <br />
    </div>
  );

  // Narratives for this component. Not pulled out because too trivial
  function setUserName(userName: string) {
    setCredentials({ ...credentials, userName });
  }

  function setPassword(password: string) {
    setCredentials({ ...credentials, password });
  }
}
