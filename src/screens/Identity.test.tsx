// Tests the VM identity panel against the MOCK backend (src/lib/mock.ts).
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Identity } from "./Identity";
import { createMockApi } from "../lib/mock";
import type { Api } from "../lib/api";
import type { SetupState } from "../lib/types";

async function mockWithVm(): Promise<{ api: Api; state: SetupState }> {
  const api = createMockApi({ delayMs: 0 });
  await api.setChoices({
    vm_name: "Windows 11",
    base_folder: null,
    ram_mb: 8192,
    cpus: 4,
    disk_gb: 100,
    iso_path: "/Users/you/Downloads/Win11_Arm64.iso",
    iso_source: "existing",
  });
  await api.createVm();
  return { api, state: await api.getState() };
}

describe("VM identity panel (mock backend)", () => {
  it("is off by default and always lists what it cannot hide", async () => {
    const { api, state } = await mockWithVm();
    render(<Identity api={api} state={state} onChanged={async () => {}} />);

    const toggle = (await screen.findByLabelText(/Enable VM identity configuration/)) as HTMLInputElement;
    expect(toggle.checked).toBe(false);
    // The "what this cannot hide" panel is always shown, with the Guest Additions caveat.
    expect(await screen.findByRole("heading", { name: /What this cannot hide/ })).toBeInTheDocument();
    await waitFor(() => expect(screen.getByText(/Guest Additions/)).toBeInTheDocument());
  });

  it("saves a configuration and previews its effects", async () => {
    const { api, state } = await mockWithVm();
    render(<Identity api={api} state={state} onChanged={async () => {}} />);

    fireEvent.click(await screen.findByLabelText(/Enable VM identity configuration/));
    fireEvent.change(await screen.findByLabelText(/Product name/), { target: { value: "OptiPlex 7090" } });
    fireEvent.change(screen.getByLabelText(/MAC address/), { target: { value: "3C5282ABCDEF" } });

    // Preview reflects the changes.
    await waitFor(() => expect(screen.getByText(/Sets the adapter MAC address/)).toBeInTheDocument());

    fireEvent.click(screen.getByRole("button", { name: "Save identity settings" }));
    await waitFor(() => expect(screen.getByText(/Identity configuration saved/)).toBeInTheDocument());
  });

  it("rejects an invalid MAC address on save", async () => {
    const { api, state } = await mockWithVm();
    render(<Identity api={api} state={state} onChanged={async () => {}} />);

    fireEvent.click(await screen.findByLabelText(/Enable VM identity configuration/));
    fireEvent.change(screen.getByLabelText(/MAC address/), { target: { value: "NOTHEX" } });
    fireEvent.click(screen.getByRole("button", { name: "Save identity settings" }));
    await waitFor(() => expect(screen.getByText(/MAC address must be 12 hexadecimal digits/)).toBeInTheDocument());
  });

  it("can apply and revert against a created VM", async () => {
    const { api } = await mockWithVm();
    // Enable + save first so apply is allowed.
    await api.setIdentityConfig({
      ...(await api.getIdentityConfig()),
      enabled: true,
      system: { ...(await api.getIdentityConfig()).system, product_name: "OptiPlex 7090" },
    });
    const state = await api.getState();
    render(<Identity api={api} state={state} onChanged={async () => {}} />);

    fireEvent.click(await screen.findByRole("button", { name: "Apply to this VM now" }));
    await waitFor(() => expect(screen.getByText(/The VM identity was applied/)).toBeInTheDocument());
  });
});
