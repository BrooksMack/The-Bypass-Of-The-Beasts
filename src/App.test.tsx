// Workflow test against the MOCK backend (src/lib/mock.ts). It proves the UI flow wiring,
// not that a real Windows installation works.
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { describe, expect, it, beforeEach } from "vitest";
import App from "./App";
import { setApiForTests } from "./lib/api";
import { createMockApi } from "./lib/mock";

describe("guided flow (mock backend)", () => {
  beforeEach(() => {
    setApiForTests(createMockApi({ delayMs: 0 }));
    window.prompt = () => "/Users/you/Downloads/Win11_25H2_English_Arm64.iso";
    window.confirm = () => true;
  });

  it("walks from welcome to the dashboard", async () => {
    render(<App />);
    expect(await screen.findByText(/MOCK BACKEND/)).toBeInTheDocument();
    fireEvent.click(await screen.findByRole("button", { name: "Set up Windows" }));

    // Check this computer
    expect(await screen.findByText(/This computer can run a Windows 11 virtual machine/)).toBeInTheDocument();
    expect(screen.getByText(/ARM 64-bit/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));

    // Choose setup
    expect(await screen.findByRole("heading", { name: "Choose setup" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "I already have the ISO" }));
    await waitFor(() => expect(screen.getByText(/Looks right for this computer/)).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: "Save and continue" }));

    // Obtain files
    expect(await screen.findByRole("heading", { name: "Get required files" })).toBeInTheDocument();
    expect(screen.getByText(/download.virtualbox.org/)).toBeInTheDocument();
    fireEvent.click(await screen.findByRole("button", { name: "Download VirtualBox" }));
    await waitFor(() => expect(screen.getByText(/VirtualBox installer downloaded and verified/)).toBeInTheDocument());
    expect(screen.getByText(/Windows 11 ISO selected/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));

    // Install dependencies
    expect(await screen.findByRole("heading", { name: "Install VirtualBox" })).toBeInTheDocument();
    fireEvent.click(await screen.findByRole("button", { name: "Open the VirtualBox installer" }));
    await waitFor(() => expect(screen.getByText(/is installed and working/)).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));

    // Create VM
    expect(await screen.findByRole("heading", { name: "Create the virtual machine" })).toBeInTheDocument();
    expect(screen.getByText(/UEFI with Secure Boot/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Create the virtual machine" }));
    await waitFor(() => expect(screen.getByText(/is ready/)).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: "Continue to Windows Setup" }));

    // Install Windows
    expect(await screen.findByRole("heading", { name: "Install Windows" })).toBeInTheDocument();
    expect(screen.getByText(/Press any key to boot/)).toBeInTheDocument();
    fireEvent.click(await screen.findByRole("button", { name: "Start Windows Setup" }));
    await screen.findByText(/Starting Windows in its own window/);
    await waitFor(() => expect(screen.getByRole("button", { name: "I reached the Windows desktop" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "I reached the Windows desktop" }));

    // Finish and verify
    expect(await screen.findByRole("heading", { name: "Finish and verify" })).toBeInTheDocument();
    expect(screen.getByText(/an address alone does not prove it/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "A web page loaded in Windows" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "A web page loaded in Windows" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "Go to everyday use" }));

    // Dashboard
    await waitFor(() => expect(screen.getByRole("heading", { name: "Windows 11" })).toBeInTheDocument());
    expect(screen.getByText(/Closing this assistant/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("tab", { name: "Troubleshooting" }));
    fireEvent.click(screen.getByRole("button", { name: "Check the network" }));
    await waitFor(() => expect(screen.getByText(/you confirmed a web page loads/)).toBeInTheDocument());
  });

  it("rejects an ISO of the wrong architecture", async () => {
    window.prompt = () => "/Users/you/Downloads/Win11_x64.iso";
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Set up Windows" }));
    fireEvent.click(await screen.findByRole("button", { name: "Continue" }));
    fireEvent.click(await screen.findByRole("button", { name: "I already have the ISO" }));
    expect(await screen.findByText(/this computer needs the ARM64 version/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save and continue" })).toBeDisabled();
  });
});
