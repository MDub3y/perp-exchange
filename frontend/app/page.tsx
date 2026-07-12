'use client';

import React from 'react';
import { ArrowUpRight, ArrowDownRight, Globe, Coins, TrendingUp, ChevronLeft, ChevronRight } from 'lucide-react';
import { useExchangeStore, MarketCategory } from '@/store/useExchangeStore';

export default function LandingPage() {
  const { activeCategory, setActiveCategory } = useExchangeStore();

  const mockEquities = [
    { name: "S&P 500", ticker: "SPY", price: 754.89, change: "+0.42%", up: true, path: "M1,39 C5,40 15,37 30,33 C45,37 60,33 75,30 C90,27 120,22 150,18" },
    { name: "Nasdaq", ticker: "QQQ", price: 726.37, change: "+0.41%", up: true, path: "M1,41 C10,42 20,38 35,35 C55,38 75,27 100,20 C125,16 150,15 175,16" },
    { name: "Gold", ticker: "GLD", price: 377.75, change: "-0.07%", up: false, path: "M1,27 C5,29 15,34 30,26 C50,30 65,42 85,38 C115,22 150,17 194,18" },
  ];

  const cryptoAssets = [
    { name: "BILL", type: "-PERP", price: 0.04791, change: "+19.89%", up: true, src: "https://backpack.exchange/coins/bill.svg" },
    { name: "ZEC", type: "-PERP", price: 546.29, change: "+7.57%", up: true, src: "https://backpack.exchange/coins/zec.png" },
    { name: "BTC", type: "-PERP", price: 64114.60, change: "-0.26%", up: false, src: "https://backpack.exchange/coins/btc.png" },
    { name: "SOL", type: "-PERP", price: 77.54, change: "-0.64%", up: false, src: "https://backpack.exchange/coins/sol.png" },
  ];

  return (
    <div className="flex flex-col flex-1 gap-8 py-4">
      <div className="flex flex-col mx-auto w-full max-w-7xl flex-1 gap-6 px-3 sm:px-6">

        {/* DYNAMIC BRAND CAROUSEL BANNER HERO BLOCK */}
        <div className="relative h-[260px] overflow-hidden rounded-[12px] border border-[#1c1e22] bg-[#121418] shadow-sm sm:h-[280px]">
          <div className="absolute inset-0 opacity-[0.85]" style={{
            background: `radial-gradient(circle 36% at 22% 22%, rgba(227, 62, 63, 0.25) 0%, transparent 54%),
                        radial-gradient(circle 42% at 86% 50%, rgba(2, 132, 199, 0.2) 0%, transparent 56%)`
          }} />

          <div className="relative flex h-full flex-col justify-center px-6 sm:px-20 z-10">
            <p className="text-[#f4f4f6] text-2xl sm:text-3xl font-bold tracking-tight">MXD Helix Execution Spine Is Live</p>
            <p className="text-[#9b9fae] mt-2 text-sm sm:text-base max-w-lg font-normal">
              Clear digital assets under sub-millisecond execution matching pipelines. Built to scale your trading parameters smoothly under the <span className="text-[#f4f4f6] font-semibold">MXDUB</span> brand umbrella.
            </p>
            <div className="flex flex-row mt-6 gap-3">
              <a className="bg-[#fd4b4e] text-white px-5 py-2.5 rounded-xl text-sm font-semibold hover:opacity-90 transition-opacity" href="/trade/SOL_USD_PERP">
                Launch Futures Desk
              </a>
            </div>
          </div>

          <ChevronLeft className="absolute top-1/2 left-3 h-8 w-8 -translate-y-1/2 text-[#5d606f] hover:text-white cursor-pointer hidden sm:block" />
          <ChevronRight className="absolute top-1/2 right-3 h-8 w-8 -translate-y-1/2 text-[#5d606f] hover:text-white cursor-pointer hidden sm:block" />
        </div>

        {/* EQUITIES WATCHLIST WIDGET MATRIX ROW */}
        <div className="grid grid-cols-1 gap-3 pt-px md:grid-cols-3">
          {mockEquities.map((stock) => (
            <div key={stock.ticker} className="flex bg-[#121418] border border-[#1c1e22] hover:border-[#282b30] p-4 rounded-xl items-center justify-between transition-all">
              <div className="flex flex-col justify-between h-full gap-1">
                <div className="flex items-center gap-2">
                  <span className="text-[#f4f4f6] font-medium text-base">{stock.name}</span>
                  <span className="text-[#9b9fae] text-sm">{stock.ticker}</span>
                </div>
                <p className="text-[#f4f4f6] text-lg font-medium font-mono">${stock.price.toFixed(2)}</p>
                <div className={`flex items-center gap-1 rounded-lg px-2 py-0.5 text-xs font-medium w-fit ${stock.up ? 'bg-[#00c278]/10 text-[#00c278]' : 'bg-[#fd4b4e]/10 text-[#fd4b4e]'}`}>
                  {stock.up ? <ArrowUpRight className="h-3.5 w-3.5" /> : <ArrowDownRight className="h-3.5 w-3.5" />}
                  {stock.change}
                </div>
              </div>
              <div className="w-[110px] h-[45px]">
                <svg className="w-full h-full overflow-visible">
                  <path d={stock.path} fill="none" stroke={stock.up ? '#00c278' : '#fd4b4e'} strokeWidth="1.5" />
                </svg>
              </div>
            </div>
          ))}
        </div>

        {/* ECOSYSTEM CATEGORIES & ASSET DISCOVERY TABLE */}
        <div className="grid w-full items-start gap-4 grid-cols-1 lg:grid-cols-3 mt-4">

          {/* Left Grid Panel Item Column: Market Categories List */}
          <div className="bg-[#121418] border border-[#1c1e22] rounded-xl p-4 flex flex-col gap-3 h-full">
            <div className="flex justify-between items-baseline mb-2">
              <p className="text-[#f4f4f6] font-semibold text-sm inline-flex gap-2 items-center">
                <TrendingUp className="w-4 h-4 text-[#00c278]" /> Top Movers
              </p>
              <span className="text-[#9b9fae] text-xs">24h Change</span>
            </div>

            {cryptoAssets.map((asset) => (
              <a key={asset.name} href={`/trade/${asset.name}_USD_PERP`} className="flex items-center justify-between p-2 rounded-lg hover:bg-white/[0.02] transition-colors group">
                <div className="flex items-center gap-2">
                  <img src={asset.src} alt={asset.name} className="w-6 h-6 rounded-full bg-[#1c1e22]" onError={(e) => { (e.target as HTMLElement).style.display = 'none'; }} />
                  <span className="font-semibold text-sm text-[#f4f4f6]">
                    {asset.name}<span className="text-[#9b9fae] font-normal">{asset.type}</span>
                  </span>
                </div>
                <div className="text-right">
                  <p className="text-[#f4f4f6] text-sm font-medium font-mono">${asset.price.toLocaleString()}</p>
                  <p className={`text-xs font-semibold font-mono ${asset.up ? 'text-[#00c278]' : 'text-[#fd4b4e]'}`}>{asset.change}</p>
                </div>
              </a>
            ))}
          </div>

          {/* Right Area Grid Spans Panel: Central Exchange Registry Sheet */}
          <div className="lg:col-span-2 border border-[#1c1e22] bg-[#121418] rounded-xl p-4 flex flex-col gap-4 shadow-sm">
            <div className="flex flex-row items-center border-b border-[#1c1e22] pb-2">
              <div className="flex gap-2">
                {(['Spot', 'Futures', 'Stocks', 'Lend'] as MarketCategory[]).map((cat) => (
                  <button
                    key={cat}
                    onClick={() => setActiveCategory(cat)}
                    className={`px-4 h-8 text-[13px] font-semibold rounded-lg transition-colors ${activeCategory === cat ? 'bg-[#1c1e22] text-[#f4f4f6]' : 'text-[#9b9fae] hover:text-[#f4f4f6]'
                      }`}
                  >
                    {cat}
                  </button>
                ))}
              </div>
            </div>

            <div className="overflow-x-auto no-scrollbar">
              <table className="min-w-full text-sm text-right">
                <thead>
                  <tr className="text-xs text-[#9b9fae] border-b border-[#1c1e22]">
                    <th className="text-left py-3 font-normal">Market Name</th>
                    <th className="py-3 font-normal">Index Spot Price</th>
                    <th className="py-3 font-normal">24h Vol (USD)</th>
                    <th className="py-3 pr-2 font-normal">24h Change</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-[#1c1e22]">
                  {marketListingsConfig.map((item) => (
                    <tr key={item.name} className="hover:bg-white/[0.02] cursor-pointer transition-colors">
                      <td className="py-4 text-left font-semibold text-[#f4f4f6]">
                        {item.name}<span className="text-[#9b9fae] font-normal">{item.label}</span>
                      </td>
                      <td className="py-4 font-mono font-medium tabular-nums">${item.price.toLocaleString(undefined, { minimumFractionDigits: 2 })}</td>
                      <td className="py-4 font-mono font-medium text-[#9b9fae]">{item.vol}</td>
                      <td className={`py-4 pr-2 font-mono font-semibold ${item.up ? 'text-[#00c278]' : 'text-[#fd4b4e]'}`}>{item.change}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>

        </div>

        {/* MODULAR COMPREHENSIVE FOOTER PLATFORM LOGS SUMMARY */}
        <div className="border-t border-[#1c1e22] mt-8 pt-8 pb-4">
          <div className="grid grid-cols-2 gap-x-6 gap-y-6 text-xs lg:grid-cols-6 text-[#9b9fae]">
            <div className="flex flex-col gap-2">
              <p className="text-[#f4f4f6] font-semibold">MXDUB Network</p>
              <a href="#" className="hover:text-[#f4f4f6]">Ecosystem Overview</a>
              <a href="#" className="hover:text-[#f4f4f6]">MXD Helix Infrastructure</a>
              <a href="#" className="hover:text-[#f4f4f6]">Contact Core Engineers</a>
            </div>
            <div className="flex flex-col gap-2">
              <p className="text-[#f4f4f6] font-semibold">Developers</p>
              <a href="#" className="hover:text-[#f4f4f6]">Clearance Spec Sheets</a>
              <a href="#" className="hover:text-[#f4f4f6]">Pipeline Metrics API</a>
              <a href="#" className="hover:text-[#f4f4f6]">System Status Spire</a>
            </div>
            <div className="col-span-2 lg:col-span-4 flex flex-col items-end justify-between">
              <div className="flex gap-4 items-center">
                <span className="text-xs text-[#f4f4f6] font-semibold">MXDUB Hub Ecosystem © 2026</span>
              </div>
              <p className="text-2xs text-[#5d606f] mt-4 text-right max-w-md leading-relaxed">
                MXD Helix is a derivatives trading matching architecture. Clearing leveraged perp tokens handles high precision isolated risk lines. Balance limits apply.
              </p>
            </div>
          </div>
        </div>

      </div>
    </div>
  );
}

const marketListingsConfig = [
  { name: "BTC", label: "-PERP", price: 64114.60, vol: "$42.4M", change: "-0.26%", up: false },
  { name: "SOL", label: "-PERP", price: 77.54, vol: "$19.8M", change: "-0.64%", up: false },
  { name: "ETH", label: "-PERP", price: 1820.60, vol: "$21.2M", change: "-0.23%", up: false },
  { name: "HYPE", label: "-PERP", price: 68.056, vol: "$8.6M", change: "+0.87%", up: true },
];