import * as React from "react";
import { useState } from "react";

import { ButtonBar, OkButton } from "./Buttons";

type Props = {
  userName: string;
  password: string;
  onOk: (userName: string, password: string) => void;
};

export function LoginView(props: Props) {
  const [credentials, setCredentials] = useState({
    userName: props.userName,
    password: props.password,
  });

  return (
    <div className="w3-card">
      <div className="w3-container w3-green">
        <h2>Login</h2>
      </div>
      <div className="w3-container">
        <br />
        <label>User Name</label>
        <input
          className="w3-input"
          type="text"
          onChange={(event) =>
            setUserName((event.target as HTMLInputElement).value)
          }
          value={credentials.userName}
        />
        <br />
        <label>Password</label>
        <input
          className="w3-input"
          type="password"
          onChange={(event) =>
            setPassword((event.target as HTMLInputElement).value)
          }
          value={credentials.password}
        />
        <br />
        {credentials.userName && credentials.password ? (
          <ButtonBar>
            <OkButton
              width="26%"
              onClick={() =>
                props.onOk(credentials.userName, credentials.password)
              }
            />
          </ButtonBar>
        ) : null}
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
