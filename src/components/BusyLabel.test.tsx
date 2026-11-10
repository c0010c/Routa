import { render, screen } from "@testing-library/react";
import { Button } from "@mui/material";
import { BusyLabel } from "./BusyLabel";

describe("BusyLabel", () => {
  it("shows only the current wording and reserves the other one's width", () => {
    const { rerender } = render(
      <Button>
        <BusyLabel busy={false} idle="刷新" pending="刷新中…" />
      </Button>,
    );
    const button = screen.getByRole("button");
    expect(button).toHaveTextContent(/^刷新$/);
    expect(button.querySelector("[data-reserve]")).toHaveAttribute("data-reserve", "刷新中…");

    rerender(
      <Button>
        <BusyLabel busy idle="刷新" pending="刷新中…" />
      </Button>,
    );
    expect(button).toHaveAccessibleName("刷新中…");
    expect(button.querySelector("[data-reserve]")).toHaveAttribute("data-reserve", "刷新");
  });
});
