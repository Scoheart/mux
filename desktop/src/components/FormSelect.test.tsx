import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it } from "vitest";
import { useState } from "react";
import { FormSelect } from "./FormSelect";

const options = [
  { value: "one", label: "One" },
  { value: "two", label: "Two" },
  { value: "three", label: "Three" },
];

function Harness() {
  const [value, setValue] = useState("one");
  return <FormSelect ariaLabel="Example" value={value} options={options} onChange={setValue} />;
}

afterEach(cleanup);

it("selects an option without a native platform popup", async () => {
  const user = userEvent.setup();
  const { container } = render(<Harness />);
  const combobox = screen.getByRole("combobox", { name: "Example" });

  expect(container.querySelector("select")).toBeNull();
  expect(combobox).toHaveTextContent("One");
  await user.click(combobox);
  await user.click(screen.getByRole("option", { name: "Two" }));

  expect(combobox).toHaveTextContent("Two");
  expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  expect(combobox).toHaveFocus();
});

it("supports arrow, Home, End, Enter, Escape, and Tab keyboard behavior", async () => {
  const user = userEvent.setup();
  render(<Harness />);
  const combobox = screen.getByRole("combobox", { name: "Example" });

  combobox.focus();
  await user.keyboard("{ArrowDown}{ArrowDown}{Enter}");
  expect(combobox).toHaveTextContent("Two");

  await user.keyboard("{ArrowUp}{Home}{End}{Enter}");
  expect(combobox).toHaveTextContent("Three");

  await user.keyboard("{Enter}{Escape}");
  expect(screen.queryByRole("listbox")).not.toBeInTheDocument();

  await user.keyboard("{Enter}{Tab}");
  expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
});
