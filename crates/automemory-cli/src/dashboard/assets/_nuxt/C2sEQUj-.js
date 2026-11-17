function n(r){if(!r)return"Note";const e=r.toLowerCase();return e==="decision"?"Decision":e==="lesson"?"Lesson":e==="gotcha"?"Watch out":e==="preference"?"Preference":e==="convention"?"Convention":r.charAt(0).toUpperCase()+r.slice(1)}function o(r){if(!r)return"";const e=new Date(r),t=Math.floor((Date.now()-e.getTime())/1e3);return t<60?"just now":t<3600?`${Math.floor(t/60)}m ago`:t<86400?`${Math.floor(t/3600)}h ago`:t<604800?`${Math.floor(t/86400)}d ago`:e.toLocaleDateString()}function i(r){const e=(r.content||"").replace(/```[\s\S]*?```/g," ").replace(/^#{1,6}\s*/gm,"").replace(/\*\*|__|`/g,"").replace(/^\s*[-*]\s+/gm,"• ").replace(/\n{3,}/g,`

`).trim();return r.type?.toLowerCase()!=="decision"?e:e.split(`
`).filter(t=>!/^(Architectural Decision|Decision):/i.test(t.trim())).map(t=>t.replace(/^Rationale:/i,"Why:")).join(`
`).replace(/\n{3,}/g,`

`).trim()}export{n as a,o as f,i as p};
