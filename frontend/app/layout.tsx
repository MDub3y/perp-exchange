import type { Metadata } from "next";
import Navbar from "@/components/Navbar";
import "./globals.css";

export const metadata: Metadata = {
  title: "MXD Helix Perps | Ecosystem Hub",
  description: "Next generation digital derivatives orderbook matching execution pipeline network.",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body className="antialiased select-none no-scrollbar bg-[#0b0e11] text-[#f4f4f6]">
        <div className="flex flex-col max-h-screen min-h-screen overflow-x-hidden overflow-y-hidden">
          <Navbar />
          <div className="bg-[#0b0e11] text-[#f4f4f6] flex flex-1 flex-col justify-between overflow-x-hidden overflow-y-auto mt-0 md:mt-20">
            {children}
          </div>
        </div>
      </body>
    </html>
  );
}