export function PromptView(props: { value: string }) {
  return (
    <header className="w3-container">
      <h3>{props.value}</h3>
    </header>
  );
}
