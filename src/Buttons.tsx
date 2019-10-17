import * as React from "react";
import { ReactNode } from "react";

type ButtonProps = { key?: string; width?: string; onClick: () => void };

export function ButtonBar(props: { children: ReactNode }) {
  return (
    <div className="w3-container">
      <div className="w3-bar">{props.children}</div>
    </div>
  );
}

export function ShowButton(props: ButtonProps) {
  return <TextButton {...props} text={"Show"} />;
}

export function BackButton(props: ButtonProps) {
  return <TextButton {...props} text={"Back"} />;
}

export function CancelButton(props: ButtonProps) {
  return <W3CssButton {...props} w3CssColor={"w3-red"} text="Cancel" />;
}

export function CreateButton(props: ButtonProps) {
  return <W3CssButton {...props} w3CssColor={"w3-green"} text="Create" />;
}

export function EditButton(props: ButtonProps) {
  return <TextButton {...props} text={"Edit"} />;
}

export function OkButton(props: ButtonProps) {
  return <W3CssButton {...props} w3CssColor={"w3-green"} text="Ok" />;
}

export function FailedButton(props: ButtonProps) {
  return <W3CssButton {...props} w3CssColor={"w3-red"} text="Failed" />;
}

export function SaveButton(props: ButtonProps) {
  return <W3CssButton {...props} w3CssColor={"w3-green"} text="Save" />;
}

export function AsNewButton(props: ButtonProps) {
  return <W3CssButton {...props} w3CssColor={"w3-green"} text="As&nbsp;new" />;
}

export function DeleteButton(props: ButtonProps) {
  return <W3CssButton {...props} w3CssColor={"w3-red"} text="Delete" />;
}

export function TextButton(props: ButtonProps & { text: string }) {
  return <W3CssButton {...props} w3CssColor={"w3-dark-grey"} />;
}

export function EnableButton(props: ButtonProps) {
  return <W3CssButton {...props} w3CssColor={"w3-green"} text="Enable" />;
}

export function DisableButton(props: ButtonProps) {
  return <W3CssButton {...props} w3CssColor={"w3-red"} text="Disable" />;
}

export function RefreshButton(props: ButtonProps) {
  return <TextButton {...props} text="Refresh" />;
}

function W3CssButton(
  props: ButtonProps & { w3CssColor?: string; text: string },
) {
  return (
    <button
      key={props.key}
      className={"w3-bar-item w3-button " + props.w3CssColor + " w3-border"}
      style={{ width: props.width }}
      onMouseDown={props.onClick} // onMouseDown because onClick sometime gets swallowed by previous blur
    >
      {props.text}
    </button>
  );
}
