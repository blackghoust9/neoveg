// OpenNeoveg 21 — Compiled TypeScript Studio Bundle
// Generated from frontend/src/*.ts


// ─── MODULE: src/types.ts ───
/**
 * OpenNeoveg 21 — Core TypeScript Data Models
 * Mirrors the Rust Core (`neoveg-core`), Python Backend, Frei0r, LADSPA & CorridorKey schemas.
 */

                                                                            

                                                                                 

                           
                 
         
              
            
             
                 

                         
                        
                
                 
               
          
              
             
               
            
           
                 
                    
                      
                  
                         
                      
                    
                      
                     
                                        
                      
                       
                       

                                  
             
                    
               
                   
                                                    
 

                                  
                                                             
            
            
                
                   
 

                               
                                                                
                                                                
                                                                   
                                               
                 
                 
                       
                            
                                
 

                                
                                         
                                                                  
 

                            
             
               
                  
                     
                    
                                                       
                 
                 
                 
                 
                  
                  
                        
                  
                     
                        
                          
                
                                                           
                                                             
                                                        
                                                     
                                                      
                             
                              
                                                                                        
                                                                          
                                                 
                 
                  
                        
                        
                   
                
 

                        
             
               
                          
                
                                                                                
                                                               
                                                                 
                                                        
                                                                              
                                                                         
                           
                 
                
                  
                 
                        
                                  
                        
                     
 

                                      
                                                                
             
               
                                                        
                    
                 
                     
                       
                                      
                                  
                                               
 

                                 
             
               
                
                
 

                                 
             
                
              
                
 

                                   
                  
                     
                         
                         
                
              
                   
                          
                         
 

                             
             
               
                    
               
                                                  
                                             
                   
                
                 
              
                
                      
                    
                  
                 
                   
                
 

                               
               
                   
                       
                
                 
              
                       
                      
                       
                      
                          
                             
                        
                     
                         
                         
              
                   
                    
                  
                       
                       
                         
                                
                                                        
                             
                         
                    
                       
                  
                            
                            
 


//# sourceURL=src/types.ts

// ─── MODULE: src/ui/icons.ts ───
/**
 * OpenNeoveg 21 — Drawn Vector SVG Icon Library (Qt / C++ NLE Style)
 * Replaces all button emojis with crisp geometric 12x12 / 14x14 vector SVG icons.
 * Every SVG includes explicit width/height attributes so buttons never collapse in any layout or theme.
 */
const Neoveg_ICONS                         = {
  welcomeHub: `<svg class="qt-icon neoveg-svg" width="13" height="13" viewBox="0 0 16 16"><rect x="1.5" y="2" width="13" height="12" fill="none" stroke="currentColor" stroke-width="1.4"/><line x1="1.5" y1="5.5" x2="14.5" y2="5.5" stroke="currentColor" stroke-width="1.3"/><rect x="3.5" y="7.5" width="4" height="4.5" fill="currentColor"/><line x1="9" y1="8.2" x2="12.8" y2="8.2" stroke="currentColor" stroke-width="1.3"/><line x1="9" y1="10.8" x2="12.8" y2="10.8" stroke="currentColor" stroke-width="1.3"/></svg>`,
  folderOpen: `<svg class="qt-icon neoveg-svg" width="13" height="13" viewBox="0 0 16 16"><path d="M1.5 3.5h4.5l1.5 1.8h7v7.7h-13z" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="miter"/><path d="M1.5 6.8h13" stroke="currentColor" stroke-width="1.2"/></svg>`,
  fileNew: `<svg class="qt-icon neoveg-svg" width="13" height="13" viewBox="0 0 16 16"><path d="M3 1.5h6.5l3.5 3.5v9.5h-10z" fill="none" stroke="currentColor" stroke-width="1.4"/><path d="M9.5 1.5v3.5h3.5" fill="none" stroke="currentColor" stroke-width="1.3"/></svg>`,
  save: `<svg class="qt-icon neoveg-svg" width="13" height="13" viewBox="0 0 16 16"><path d="M2 2h9.5l2.5 2.5v9.5h-12z" fill="none" stroke="currentColor" stroke-width="1.4"/><rect x="4.5" y="2" width="5" height="4" fill="none" stroke="currentColor" stroke-width="1.2"/><rect x="4" y="9" width="8" height="5" fill="none" stroke="currentColor" stroke-width="1.2"/></svg>`,
  cursorSelect: `<svg class="qt-icon neoveg-svg" width="13" height="13" viewBox="0 0 16 16"><path d="M3.5 2L12.5 8.5H7.8L10 13.5L8 14.3L5.8 9.2L3.5 11.8Z" fill="currentColor"/></svg>`,
  splitRazor: `<svg class="qt-icon neoveg-svg" width="13" height="13" viewBox="0 0 16 16"><line x1="8" y1="1" x2="8" y2="15" stroke="currentColor" stroke-width="1.5" stroke-dasharray="2 1.5"/><path d="M2 5h4v6H2zM10 5h4v6h-4z" fill="none" stroke="currentColor" stroke-width="1.3"/></svg>`,
  slipTool: `<svg class="qt-icon neoveg-svg" width="13" height="13" viewBox="0 0 16 16"><line x1="2" y1="3" x2="2" y2="13" stroke="currentColor" stroke-width="1.6"/><line x1="14" y1="3" x2="14" y2="13" stroke="currentColor" stroke-width="1.6"/><path d="M4.5 8h7M6.5 5.5L4 8l2.5 2.5M9.5 5.5L12 8l-2.5 2.5" fill="none" stroke="currentColor" stroke-width="1.3"/></svg>`,
  slideTool: `<svg class="qt-icon neoveg-svg" width="13" height="13" viewBox="0 0 16 16"><rect x="4.5" y="4.5" width="7" height="7" fill="none" stroke="currentColor" stroke-width="1.4"/><path d="M1 8h2.5M12.5 8H15M2.8 6L1 8l1.8 2M13.2 6L15 8l-1.8 2" fill="none" stroke="currentColor" stroke-width="1.3"/></svg>`,
  rateStretch: `<svg class="qt-icon neoveg-svg" width="13" height="13" viewBox="0 0 16 16"><circle cx="8" cy="8" r="5.5" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M8 4.5V8l2.8 1.8" fill="none" stroke="currentColor" stroke-width="1.4"/></svg>`,
  magnetSnap: `<svg class="qt-icon neoveg-svg" width="13" height="13" viewBox="0 0 16 16"><path d="M4 2.5v5.5a4 4 0 0 0 8 0V2.5h-2.5v5.5a1.5 1.5 0 0 1-3 0V2.5z" fill="none" stroke="currentColor" stroke-width="1.4"/><line x1="3.5" y1="5" x2="6.5" y2="5" stroke="currentColor" stroke-width="1.3"/><line x1="9.5" y1="5" x2="12.5" y2="5" stroke="currentColor" stroke-width="1.3"/></svg>`,
  rippleWave: `<svg class="qt-icon neoveg-svg" width="13" height="13" viewBox="0 0 16 16"><path d="M1.5 8h3l2-4.5 3 9 2-4.5h3" fill="none" stroke="currentColor" stroke-width="1.5"/></svg>`,
  crossfadeX: `<svg class="qt-icon neoveg-svg" width="13" height="13" viewBox="0 0 16 16"><rect x="1.5" y="3" width="13" height="10" fill="none" stroke="currentColor" stroke-width="1.2"/><line x1="2" y1="3.5" x2="14" y2="12.5" stroke="currentColor" stroke-width="1.4"/><line x1="2" y1="12.5" x2="14" y2="3.5" stroke="currentColor" stroke-width="1.4"/></svg>`,
  markerFlag: `<svg class="qt-icon neoveg-svg" width="13" height="13" viewBox="0 0 16 16"><line x1="3.5" y1="2" x2="3.5" y2="14.5" stroke="currentColor" stroke-width="1.5"/><path d="M3.5 2.5h9l-2.2 3 2.2 3h-9z" fill="currentColor"/></svg>`,
  regionBrackets: `<svg class="qt-icon neoveg-svg" width="13" height="13" viewBox="0 0 16 16"><path d="M5 2.5H2.5v11H5M11 2.5h2.5v11H11" fill="none" stroke="currentColor" stroke-width="1.6"/><line x1="5" y1="8" x2="11" y2="8" stroke="currentColor" stroke-width="1.2" stroke-dasharray="1.5 1.5"/></svg>`,
  trash: `<svg class="qt-icon neoveg-svg" width="13" height="13" viewBox="0 0 16 16"><path d="M3.5 4.5h9l-.7 9.5H4.2z" fill="none" stroke="currentColor" stroke-width="1.3"/><line x1="2" y1="4.5" x2="14" y2="4.5" stroke="currentColor" stroke-width="1.4"/><path d="M6 2.5h4v2H6z" fill="none" stroke="currentColor" stroke-width="1.2"/></svg>`,
  floatWindow: `<svg class="qt-icon neoveg-svg" width="12" height="12" viewBox="0 0 16 16"><rect x="4.5" y="2" width="9.5" height="8.5" fill="none" stroke="currentColor" stroke-width="1.5"/><path d="M2 5.5v8.5h9.5" fill="none" stroke="currentColor" stroke-width="1.4"/></svg>`,
  dockWindow: `<svg class="qt-icon neoveg-svg" width="12" height="12" viewBox="0 0 16 16"><rect x="1.5" y="2" width="13" height="12" fill="none" stroke="currentColor" stroke-width="1.5"/><line x1="6.5" y1="2" x2="6.5" y2="14" stroke="currentColor" stroke-width="1.4"/><line x1="6.5" y1="8" x2="14.5" y2="8" stroke="currentColor" stroke-width="1.4"/></svg>`,
  minimize: `<svg class="qt-icon neoveg-svg" width="11" height="11" viewBox="0 0 12 12"><line x1="2" y1="6" x2="10" y2="6" stroke="currentColor" stroke-width="1.8"/></svg>`,
  maximize: `<svg class="qt-icon neoveg-svg" width="11" height="11" viewBox="0 0 12 12"><rect x="2" y="2" width="8" height="8" fill="none" stroke="currentColor" stroke-width="1.5"/></svg>`,
  restore: `<svg class="qt-icon neoveg-svg" width="11" height="11" viewBox="0 0 12 12"><rect x="3.5" y="1.5" width="6.5" height="6.5" fill="none" stroke="currentColor" stroke-width="1.3"/><rect x="1.5" y="3.5" width="6.5" height="6.5" fill="none" stroke="currentColor" stroke-width="1.4"/></svg>`,
  close: `<svg class="qt-icon neoveg-svg" width="11" height="11" viewBox="0 0 12 12"><line x1="2" y1="2" x2="10" y2="10" stroke="currentColor" stroke-width="1.6"/><line x1="10" y1="2" x2="2" y2="10" stroke="currentColor" stroke-width="1.6"/></svg>`,
  terminalCode: `<svg class="qt-icon neoveg-svg" width="13" height="13" viewBox="0 0 16 16"><rect x="1.5" y="2.5" width="13" height="11" fill="none" stroke="currentColor" stroke-width="1.4"/><path d="M4 6l3 2.2L4 10.4M8.5 10.5h3.5" fill="none" stroke="currentColor" stroke-width="1.4"/></svg>`,
  renderFilm: `<svg class="qt-icon neoveg-svg" width="13" height="13" viewBox="0 0 16 16"><rect x="2" y="2" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.4"/><line x1="5" y1="2" x2="5" y2="14" stroke="currentColor" stroke-width="1.2"/><line x1="11" y1="2" x2="11" y2="14" stroke="currentColor" stroke-width="1.2"/><line x1="2" y1="8" x2="14" y2="8" stroke="currentColor" stroke-width="1.2"/></svg>`,
  shieldCheck: `<svg class="qt-icon neoveg-svg" width="13" height="13" viewBox="0 0 16 16"><path d="M8 1.5l5.5 2.2v4.3c0 3.5-2.4 5.6-5.5 6.8-3.1-1.2-5.5-3.3-5.5-6.8V3.7z" fill="none" stroke="currentColor" stroke-width="1.4"/><path d="M5.5 8l1.8 1.8 3.5-3.6" fill="none" stroke="currentColor" stroke-width="1.5"/></svg>`,
  play: `<svg class="qt-icon neoveg-svg" width="12" height="12" viewBox="0 0 14 14"><polygon points="3,2 12,7 3,12" fill="currentColor"/></svg>`,
  pause: `<svg class="qt-icon neoveg-svg" width="12" height="12" viewBox="0 0 14 14"><rect x="3" y="2" width="3" height="10" fill="currentColor"/><rect x="8" y="2" width="3" height="10" fill="currentColor"/></svg>`,
  stop: `<svg class="qt-icon neoveg-svg" width="12" height="12" viewBox="0 0 14 14"><rect x="2.5" y="2.5" width="9" height="9" fill="currentColor"/></svg>`,
  stepStart: `<svg class="qt-icon neoveg-svg" width="12" height="12" viewBox="0 0 14 14"><rect x="2" y="2.5" width="2" height="9" fill="currentColor"/><polygon points="11.5,2.5 5,7 11.5,11.5" fill="currentColor"/></svg>`,
  stepEnd: `<svg class="qt-icon neoveg-svg" width="12" height="12" viewBox="0 0 14 14"><rect x="10" y="2.5" width="2" height="9" fill="currentColor"/><polygon points="2.5,2.5 9,7 2.5,11.5" fill="currentColor"/></svg>`,
  stepPrev: `<svg class="qt-icon neoveg-svg" width="12" height="12" viewBox="0 0 14 14"><polygon points="9.5,2.5 3.5,7 9.5,11.5" fill="currentColor"/></svg>`,
  stepNext: `<svg class="qt-icon neoveg-svg" width="12" height="12" viewBox="0 0 14 14"><polygon points="4.5,2.5 10.5,7 4.5,11.5" fill="currentColor"/></svg>`,
  loop: `<svg class="qt-icon neoveg-svg" width="12" height="12" viewBox="0 0 16 16"><path d="M3 6V4h9l-1.8-1.8M13 10v2H4l1.8 1.8" fill="none" stroke="currentColor" stroke-width="1.5"/></svg>`,
  splitScreen: `<svg class="qt-icon neoveg-svg" width="12" height="12" viewBox="0 0 14 14"><rect x="1.5" y="2" width="11" height="10" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M1.5 2h5.5v10H1.5z" fill="currentColor" fill-opacity="0.35"/><line x1="7" y1="1.5" x2="7" y2="12.5" stroke="currentColor" stroke-width="1.4"/></svg>`,
  cropFrame: `<svg class="qt-icon neoveg-svg" width="12" height="12" viewBox="0 0 14 14"><path d="M3.5 1v9.5H13M1 3.5h9.5V13" fill="none" stroke="currentColor" stroke-width="1.4"/></svg>`,
  grid: `<svg class="qt-icon neoveg-svg" width="12" height="12" viewBox="0 0 14 14"><rect x="1.5" y="1.5" width="11" height="11" fill="none" stroke="currentColor" stroke-width="1.2"/><line x1="5.2" y1="1.5" x2="5.2" y2="12.5" stroke="currentColor" stroke-width="1.1"/><line x1="8.8" y1="1.5" x2="8.8" y2="12.5" stroke="currentColor" stroke-width="1.1"/><line x1="1.5" y1="5.2" x2="12.5" y2="5.2" stroke="currentColor" stroke-width="1.1"/><line x1="1.5" y1="8.8" x2="12.5" y2="8.8" stroke="currentColor" stroke-width="1.1"/></svg>`,
  safeArea: `<svg class="qt-icon neoveg-svg" width="12" height="12" viewBox="0 0 14 14"><rect x="1.5" y="2" width="11" height="10" fill="none" stroke="currentColor" stroke-width="1.2"/><rect x="3.5" y="3.8" width="7" height="6.4" fill="none" stroke="currentColor" stroke-width="1.1" stroke-dasharray="1.5 1"/></svg>`,
  camera: `<svg class="qt-icon neoveg-svg" width="12" height="12" viewBox="0 0 14 14"><path d="M1.5 4h2.2l1-1.5h4.6l1 1.5h2.2v7.5h-11z" fill="none" stroke="currentColor" stroke-width="1.3"/><circle cx="7" cy="7.8" r="2.2" fill="none" stroke="currentColor" stroke-width="1.3"/></svg>`,
  colorWheels: `<svg class="qt-icon neoveg-svg" width="13" height="13" viewBox="0 0 16 16"><circle cx="5.2" cy="6" r="3.2" fill="none" stroke="currentColor" stroke-width="1.3"/><circle cx="10.8" cy="6" r="3.2" fill="none" stroke="currentColor" stroke-width="1.3"/><circle cx="8" cy="10.8" r="3.2" fill="none" stroke="currentColor" stroke-width="1.3"/></svg>`,
  corridorKey: `<svg class="qt-icon neoveg-svg" width="13" height="13" viewBox="0 0 16 16"><rect x="1.5" y="2" width="13" height="12" fill="none" stroke="currentColor" stroke-width="1.3"/><circle cx="8" cy="6.2" r="2.1" fill="currentColor"/><path d="M4.2 13c0-2.1 1.7-3.6 3.8-3.6s3.8 1.5 3.8 3.6" fill="currentColor"/></svg>`,
  plus: `<svg class="qt-icon neoveg-svg" width="11" height="11" viewBox="0 0 12 12"><line x1="6" y1="2" x2="6" y2="10" stroke="currentColor" stroke-width="1.6"/><line x1="2" y1="6" x2="10" y2="6" stroke="currentColor" stroke-width="1.6"/></svg>`,
  minus: `<svg class="qt-icon neoveg-svg" width="11" height="11" viewBox="0 0 12 12"><line x1="2" y1="6" x2="10" y2="6" stroke="currentColor" stroke-width="1.6"/></svg>`,
  popoutMonitor: `<svg class="qt-icon neoveg-svg" width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="1.5" y="3" width="10" height="7.5"/><line x1="4.5" y1="13" x2="8.5" y2="13"/><line x1="6.5" y1="10.5" x2="6.5" y2="13"/><polyline points="10 2 14.5 2 14.5 6.5"/><line x1="8.5" y1="7.5" x2="14.5" y2="2"/></svg>`,
  settingsGear: `<svg class="qt-icon neoveg-svg" width="13" height="13" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="8" cy="8" r="2.5"/><path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M12.6 3.4l-1.4 1.4M4.8 11.2l-1.4 1.4"/></svg>`,
  trackExpand: `<svg class="qt-icon neoveg-svg" width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><line x1="2" y1="2.5" x2="14" y2="2.5"/><line x1="2" y1="13.5" x2="14" y2="13.5"/><polyline points="5.5 6 8 3.8 10.5 6"/><polyline points="5.5 10 8 12.2 10.5 10"/><line x1="8" y1="4" x2="8" y2="12"/></svg>`,
  trackShrink: `<svg class="qt-icon neoveg-svg" width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><line x1="2" y1="2.5" x2="14" y2="2.5"/><line x1="2" y1="13.5" x2="14" y2="13.5"/><polyline points="5.5 4.2 8 6.8 10.5 4.2"/><polyline points="5.5 11.8 8 9.2 10.5 11.8"/></svg>`,
  renameEdit: `<svg class="qt-icon neoveg-svg" width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M11.2 2.2l2.6 2.6-8.3 8.3-3.3.7.7-3.3z"/><line x1="9.5" y1="3.9" x2="12.1" y2="6.5"/></svg>`,
  groupLink: `<svg class="qt-icon neoveg-svg" width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="2" y="2.5" width="5" height="4.5"/><rect x="2" y="9" width="5" height="4.5"/><path d="M7 4.7h4v6.6H7"/><circle cx="11" cy="8" r="1.4" fill="currentColor"/></svg>`,
};
function iconSvg(name                          , label         )         {
  const svg = Neoveg_ICONS[name] || '';
  return label ? `${svg}<span>${label}</span>` : svg;
}


//# sourceURL=src/ui/icons.ts

// ─── MODULE: src/ui/faceThumbnails.ts ───
/**
 * OpenNeoveg 21 — Human Face Demo Thumbnail Renderer for Video FX, Frei0r & CorridorKey
 * Draws a detailed human portrait (head, natural skin tones, eyes with catchlights, eyebrows,
 * nose, lips, hair, and studio backdrop) and applies the actual pixel/canvas transformation
 * of each effect so every FX card displays a live visual demonstration on a human face!
 */
function renderHumanFaceEffectThumbnail(
  canvas                   ,
  effectType        
)       {
  const w = canvas.width || 68;
  const h = canvas.height || 50;
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  if (!ctx) return;

  // Offscreen buffer for base human portrait
  const off = document.createElement('canvas');
  off.width = w;
  off.height = h;
  const octx = off.getContext('2d', { willReadFrequently: true }) ;

  // 1. Studio Backdrop (Bright Green Screen for CorridorKey, or Studio Gradient for others)
  if (effectType === 'corridorKey') {
    // Split backdrop: Left side raw green screen, Right side unmixed studio dark cyber backdrop!
    octx.fillStyle = '#00d828';
    octx.fillRect(0, 0, w / 2, h);
    const rightGrad = octx.createLinearGradient(w / 2, 0, w, h);
    rightGrad.addColorStop(0, '#0f172a');
    rightGrad.addColorStop(1, '#1e3a8a');
    octx.fillStyle = rightGrad;
    octx.fillRect(w / 2, 0, w / 2, h);
  } else {
    const bgGrad = octx.createLinearGradient(0, 0, w, h);
    bgGrad.addColorStop(0, '#1e293b');
    bgGrad.addColorStop(0.6, '#334155');
    bgGrad.addColorStop(1, '#0f172a');
    octx.fillStyle = bgGrad;
    octx.fillRect(0, 0, w, h);
  }

  const cx = w / 2;
  const cy = h * 0.48;

  // 2. Shoulders / Collar
  octx.fillStyle = '#1e3a5f';
  octx.beginPath();
  octx.ellipse(cx, h + 4, 24, 14, 0, Math.PI, 0, false);
  octx.fill();

  // 3. Neck
  octx.fillStyle = '#d49b78';
  octx.fillRect(cx - 5, cy + 10, 10, 11);

  // 4. Back Hair Silhouette
  octx.fillStyle = '#271c18';
  octx.beginPath();
  octx.ellipse(cx, cy - 3, 15.5, 17.5, 0, 0, Math.PI * 2);
  octx.fill();

  // 5. Ears
  octx.fillStyle = '#dba382';
  octx.beginPath();
  octx.ellipse(cx - 13.5, cy + 1, 2.5, 4.5, 0, 0, Math.PI * 2);
  octx.ellipse(cx + 13.5, cy + 1, 2.5, 4.5, 0, 0, Math.PI * 2);
  octx.fill();

  // 6. Human Face Oval with 3D Studio Lighting Gradient
  const skinGrad = octx.createRadialGradient(cx - 3, cy - 4, 2, cx, cy, 16);
  skinGrad.addColorStop(0, '#f6ccb2');
  skinGrad.addColorStop(0.65, '#e2a887');
  skinGrad.addColorStop(1, '#b87b5c');
  octx.fillStyle = skinGrad;
  octx.beginPath();
  octx.ellipse(cx, cy, 13, 16, 0, 0, Math.PI * 2);
  octx.fill();

  // Cheek warmth
  octx.fillStyle = 'rgba(225, 105, 95, 0.24)';
  octx.beginPath();
  octx.arc(cx - 6.5, cy + 3, 3.5, 0, Math.PI * 2);
  octx.arc(cx + 6.5, cy + 3, 3.5, 0, Math.PI * 2);
  octx.fill();

  // 7. Front Hair / Bangs
  octx.fillStyle = '#231815';
  octx.beginPath();
  octx.ellipse(cx, cy - 11.5, 13.5, 6.5, 0, Math.PI, 0, false);
  octx.fill();
  octx.beginPath();
  octx.moveTo(cx - 13, cy - 8);
  octx.quadraticCurveTo(cx - 4, cy - 15, cx + 13, cy - 7);
  octx.lineTo(cx + 13, cy - 14);
  octx.lineTo(cx - 13, cy - 14);
  octx.fill();

  // 8. Eyebrows
  octx.strokeStyle = '#36241d';
  octx.lineWidth = 1.4;
  octx.beginPath();
  octx.moveTo(cx - 8.5, cy - 4.2);
  octx.quadraticCurveTo(cx - 5.2, cy - 5.8, cx - 2.2, cy - 4.4);
  octx.moveTo(cx + 2.2, cy - 4.4);
  octx.quadraticCurveTo(cx + 5.2, cy - 5.8, cx + 8.5, cy - 4.2);
  octx.stroke();

  // 9. Eyes (Sclera + Iris + Pupil + Catchlight)
  const drawEye = (ex        , ey        ) => {
    octx.fillStyle = '#f8fafc';
    octx.beginPath();
    octx.ellipse(ex, ey, 3.1, 1.8, 0, 0, Math.PI * 2);
    octx.fill();

    octx.fillStyle = '#2563eb';
    octx.beginPath();
    octx.arc(ex, ey, 1.5, 0, Math.PI * 2);
    octx.fill();

    octx.fillStyle = '#0f172a';
    octx.beginPath();
    octx.arc(ex, ey, 0.8, 0, Math.PI * 2);
    octx.fill();

    octx.fillStyle = '#ffffff';
    octx.fillRect(ex - 0.8, ey - 1.1, 0.9, 0.9);
  };
  drawEye(cx - 5.2, cy - 1.5);
  drawEye(cx + 5.2, cy - 1.5);

  // 10. Nose bridge & tip shadow
  octx.strokeStyle = 'rgba(138, 82, 58, 0.65)';
  octx.lineWidth = 1.1;
  octx.beginPath();
  octx.moveTo(cx - 0.5, cy - 2);
  octx.lineTo(cx - 1.3, cy + 4);
  octx.lineTo(cx + 1.2, cy + 4.4);
  octx.stroke();

  // 11. Lips
  octx.fillStyle = '#c45a55';
  octx.beginPath();
  octx.ellipse(cx, cy + 8.5, 4.5, 1.9, 0, 0, Math.PI * 2);
  octx.fill();
  octx.strokeStyle = '#8c3532';
  octx.lineWidth = 0.8;
  octx.beginPath();
  octx.moveTo(cx - 4.3, cy + 8.5);
  octx.lineTo(cx + 4.3, cy + 8.5);
  octx.stroke();

  // ─── APPLY SPECIFIC VIDEO FX / FREI0R / CORRIDORKEY TO THE PORTRAIT ───
  ctx.clearRect(0, 0, w, h);
  ctx.save();

  if (effectType === 'brightnessContrast') {
    ctx.filter = 'brightness(118%) contrast(132%) saturate(118%)';
    ctx.drawImage(off, 0, 0);
  } else if (effectType === 'colorGrade' || effectType === 'frei0r.coloradj_RGB') {
    ctx.filter = 'contrast(114%) saturate(135%)';
    ctx.drawImage(off, 0, 0);
    // Teal shadows / Orange skin highlight split
    const grad = ctx.createLinearGradient(0, 0, w, h);
    grad.addColorStop(0, 'rgba(6, 182, 212, 0.28)');
    grad.addColorStop(1, 'rgba(249, 115, 22, 0.28)');
    ctx.fillStyle = grad;
    ctx.fillRect(0, 0, w, h);
  } else if (effectType === 'blur') {
    ctx.filter = 'blur(2.4px)';
    ctx.drawImage(off, 0, 0);
  } else if (effectType === 'sepia') {
    ctx.filter = 'sepia(95%) contrast(108%)';
    ctx.drawImage(off, 0, 0);
  } else if (effectType === 'invert') {
    ctx.filter = 'invert(100%)';
    ctx.drawImage(off, 0, 0);
  } else if (effectType === 'vignette') {
    ctx.drawImage(off, 0, 0);
    const vgrad = ctx.createRadialGradient(cx, cy, 8, cx, cy, w * 0.55);
    vgrad.addColorStop(0, 'rgba(0,0,0,0)');
    vgrad.addColorStop(1, 'rgba(0,0,0,0.82)');
    ctx.fillStyle = vgrad;
    ctx.fillRect(0, 0, w, h);
  } else if (effectType === 'vhsGlitch' || effectType === 'frei0r.distort0r') {
    // Draw RGB chromatic shift + scanlines
    ctx.drawImage(off, -2, 0);
    ctx.globalAlpha = 0.5;
    ctx.drawImage(off, 2, 0);
    ctx.globalAlpha = 1.0;
    ctx.fillStyle = 'rgba(0,0,0,0.28)';
    for (let y = 0; y < h; y += 3) {
      ctx.fillRect(0, y, w, 1);
    }
  } else if (effectType === 'frei0r.pixeliz0r') {
    const small = document.createElement('canvas');
    small.width = 12;
    small.height = 9;
    const sctx = small.getContext('2d') ;
    sctx.drawImage(off, 0, 0, 12, 9);
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(small, 0, 0, w, h);
  } else if (effectType === 'frei0r.glow') {
    ctx.drawImage(off, 0, 0);
    ctx.save();
    ctx.filter = 'blur(3.5px) brightness(140%)';
    ctx.globalCompositeOperation = 'screen';
    ctx.globalAlpha = 0.75;
    ctx.drawImage(off, 0, 0);
    ctx.restore();
  } else if (effectType === 'frei0r.cartoon' || effectType === 'frei0r.posterize') {
    ctx.drawImage(off, 0, 0);
    const img = ctx.getImageData(0, 0, w, h);
    const d = img.data;
    const levels = effectType === 'frei0r.cartoon' ? 4 : 3;
    const step = 255 / levels;
    for (let i = 0; i < d.length; i += 4) {
      d[i] = Math.round(d[i] / step) * step;
      d[i + 1] = Math.round(d[i + 1] / step) * step;
      d[i + 2] = Math.round(d[i + 2] / step) * step;
    }
    ctx.putImageData(img, 0, 0);
  } else if (effectType === 'frei0r.sobel') {
    const src = octx.getImageData(0, 0, w, h).data;
    const out = ctx.createImageData(w, h);
    const dst = out.data;
    for (let y = 1; y < h - 1; y++) {
      for (let x = 1; x < w - 1; x++) {
        const idx = (y * w + x) * 4;
        const gx = Math.abs(src[idx + 4] - src[idx - 4]);
        const gy = Math.abs(src[idx + w * 4] - src[idx - w * 4]);
        const mag = Math.min(255, (gx + gy) * 2.4);
        dst[idx] = mag * 0.4;
        dst[idx + 1] = mag;
        dst[idx + 2] = Math.min(255, mag * 1.2);
        dst[idx + 3] = 255;
      }
    }
    ctx.putImageData(out, 0, 0);
  } else if (effectType === 'corridorKey') {
    ctx.drawImage(off, 0, 0);
    // Split line showing Green Screen -> Clean Keyed Alpha Unmix
    ctx.strokeStyle = '#ffffff';
    ctx.lineWidth = 1.2;
    ctx.beginPath();
    ctx.moveTo(w / 2, 0);
    ctx.lineTo(w / 2, h);
    ctx.stroke();
  } else {
    ctx.drawImage(off, 0, 0);
  }

  ctx.restore();
}


//# sourceURL=src/ui/faceThumbnails.ts

// ─── MODULE: src/engine/compositor.ts ───
/**
 * OpenNeoveg — Real-Time Multi-Track Video Compositor & Scope Engine
 */
function evaluateFadeCurve(curve               , t        )         {
  const x = Math.max(0, Math.min(1, t));
  switch (curve) {
    case 'linear':
      return x;
    case 'fast':
      return 1 - Math.pow(1 - x, 3);
    case 'slow':
      return Math.pow(x, 3);
    case 'sharp':
      return x < 0.5
        ? 0.5 * Math.pow(2 * x, 3)
        : 1 - 0.5 * Math.pow(2 * (1 - x), 3);
    case 'smooth':
    default:
      return 0.5 - 0.5 * Math.cos(Math.PI * x);
  }
}
function formatSMPTE(seconds        , fps = 29.97, dropFrame = true)         {
  const clamped = Math.max(0, seconds);
  const nomFps = Math.max(1, Math.round(fps));
  const totalFrames = Math.floor(clamped * fps + 1e-4);
  const frames = totalFrames % nomFps;
  const totalSeconds = Math.floor(totalFrames / nomFps);
  const secs = totalSeconds % 60;
  const totalMinutes = Math.floor(totalSeconds / 60);
  const mins = totalMinutes % 60;
  const hours = Math.floor(totalMinutes / 60);
  const sep = dropFrame ? ';' : ':';
  return (
    String(hours).padStart(2, '0') +
    ':' +
    String(mins).padStart(2, '0') +
    ':' +
    String(secs).padStart(2, '0') +
    sep +
    String(frames).padStart(2, '0')
  );
}
function evaluateTrackEnvelope(points                 , time        , fallback        )         {
  if (!points || points.length === 0) return fallback;
  const sorted = [...points].sort((a, b) => a.time - b.time);
  if (time <= sorted[0].time) return sorted[0].value;
  if (time >= sorted[sorted.length - 1].time) return sorted[sorted.length - 1].value;
  for (let i = 0; i < sorted.length - 1; i++) {
    const p0 = sorted[i];
    const p1 = sorted[i + 1];
    if (time >= p0.time && time <= p1.time) {
      const span = Math.max(0.0001, p1.time - p0.time);
      const ratio = (time - p0.time) / span;
      return p0.value + (p1.value - p0.value) * ratio;
    }
  }
  return fallback;
}
function computeClipFadeMultiplier(clip           , playhead        )         {
  const localT = playhead - clip.start;
  if (localT < 0 || localT > clip.duration) return 0;
  let mult = clip.gain;
  if (clip.fadeIn > 0.005 && localT < clip.fadeIn) {
    mult *= evaluateFadeCurve(clip.fadeInCurve, localT / clip.fadeIn);
  }
  const rem = clip.duration - localT;
  if (clip.fadeOut > 0.005 && rem < clip.fadeOut) {
    mult *= evaluateFadeCurve(clip.fadeOutCurve, rem / clip.fadeOut);
  }
  return Math.max(0, Math.min(2, mult));
}

function mapBlendMode(mode               )                           {
  switch (mode) {
    case 'add':
      return 'lighter';
    case 'multiply':
      return 'multiply';
    case 'screen':
      return 'screen';
    case 'overlay':
      return 'overlay';
    case 'difference':
      return 'difference';
    case 'sourceAlpha':
    default:
      return 'source-over';
  }
}
class NeovegHostCompositor {
          canvas                   ;
          ctx                          ;
          offscreen                   ;
          offCtx                          ;
          scopeCanvas                           = null;
          videoPool                                = new Map();

  constructor(canvas                   , scopeCanvas                           ) {
    this.canvas = canvas;
    const context = canvas.getContext('2d', { willReadFrequently: true });
    if (!context) throw new Error('2D context unavailable');
    this.ctx = context;

    this.offscreen = document.createElement('canvas');
    this.offscreen.width = canvas.width;
    this.offscreen.height = canvas.height;
    this.offCtx = this.offscreen.getContext('2d', { willReadFrequently: true }) ;
    this.scopeCanvas = scopeCanvas || null;
  }

         registerVideoElement(url        )                   {
    let vid = this.videoPool.get(url);
    if (!vid) {
      vid = document.createElement('video');
      vid.src = url;
      vid.muted = true;
      vid.preload = 'auto';
      vid.playsInline = true;
      vid.crossOrigin = 'anonymous';
      this.videoPool.set(url, vid);
    }
    return vid;
  }

         syncVideos(project              , isPlaying         )       {
    const t = project.playhead;
    for (const track of project.tracks) {
      if (track.type !== 'video') continue;
      for (const clip of track.clips) {
        if (!clip.mediaUrl) continue;
        const vid = this.registerVideoElement(clip.mediaUrl);
        const active = t >= clip.start && t <= clip.start + clip.duration && !clip.muted && !track.muted;
        if (active) {
          const targetMediaTime = clip.trimIn + (t - clip.start) * (clip.playbackRate || 1);
          const drift = Math.abs((vid.currentTime || 0) - targetMediaTime);
          if (vid.readyState >= 1) {
            const dur = vid.duration || 6;
            const wrapped = targetMediaTime % dur;
            if (!isPlaying && drift > 0.03) {
              vid.currentTime = wrapped;
            } else if (isPlaying && drift > 0.22) {
              vid.currentTime = wrapped;
            }
            vid.playbackRate = Math.max(0.25, Math.min(4.0, clip.playbackRate || 1.0));
            if (isPlaying && vid.paused) {
              vid.play().catch(() => {});
            } else if (!isPlaying && !vid.paused) {
              vid.pause();
            }
          }
        } else if (!vid.paused) {
          vid.pause();
        }
      }
    }
  }

         renderFrame(project              , selectedClip                  , showPanCropGizmo = false)       {
    const scaleMap                                                 = {
      draft: 0.4,
      preview: 0.65,
      good: 0.85,
      best: 1.0,
    };
    const qScale = scaleMap[project.previewQuality] || 1.0;
    const targetW = Math.round(640 * qScale);
    const targetH = Math.round(360 * qScale);

    if (this.canvas.width !== targetW || this.canvas.height !== targetH) {
      this.canvas.width = targetW;
      this.canvas.height = targetH;
      this.offscreen.width = targetW;
      this.offscreen.height = targetH;
    }

    const w = this.canvas.width;
    const h = this.canvas.height;
    const ctx = this.ctx;

    // Clear master frame
    ctx.save();
    ctx.fillStyle = '#07080a';
    ctx.fillRect(0, 0, w, h);

    const anySolo = project.tracks.some((tr) => tr.type === 'video' && tr.solo);
    // In Neoveg, Track 1 (top) renders on top of Track 2 (below).
    // So we composite from bottom-most video track up to top-most video track.
    const videoTracks = project.tracks
      .filter((tr) => tr.type === 'video')
      .slice()
      .reverse();

    for (const track of videoTracks) {
      if (track.muted || (anySolo && !track.solo)) continue;
      const trackLevel = track.showEnvelope
        ? evaluateTrackEnvelope(track.envelopePoints, project.playhead, track.level)
        : track.level;
      if (trackLevel <= 0.001) continue;

      const activeClips = track.clips.filter(
        (c) => !c.muted && project.playhead >= c.start && project.playhead <= c.start + c.duration
      );

      for (const clip of activeClips) {
        this.renderClipToLayer(clip, track, trackLevel, project, w, h);
      }
    }

    // Overlays: Safe Areas, Rule-of-Thirds Grid, Pan/Crop Gizmo
    if (project.showGrid) {
      this.drawGridOverlay(ctx, w, h);
    }
    if (project.showSafeAreas) {
      this.drawSafeAreasOverlay(ctx, w, h);
    }
    if (showPanCropGizmo && selectedClip && selectedClip.mediaType !== 'audio') {
      this.drawPanCropGizmo(ctx, selectedClip, w, h);
    }

    ctx.restore();

    if (this.scopeCanvas) {
      this.updateVideoScopes();
    }
  }

          renderClipToLayer(
    clip           ,
    track       ,
    trackLevel        ,
    project              ,
    w        ,
    h        
  )       {
    const octx = this.offCtx;
    octx.save();
    octx.clearRect(0, 0, w, h);

    const fadeMult = computeClipFadeMultiplier(clip, project.playhead);
    const effectiveAlpha = Math.max(0, Math.min(1, fadeMult * trackLevel));
    if (effectiveAlpha <= 0.001) {
      octx.restore();
      return;
    }

    const localT = project.playhead - clip.start;
    const fadeProgress = clip.fadeIn > 0.01 && localT < clip.fadeIn ? localT / clip.fadeIn : 1.0;

    // Transition clip-path / transform
    if (fadeProgress < 1.0 && clip.transitionType) {
      if (clip.transitionType === 'wipeLeft') {
        octx.beginPath();
        octx.rect(0, 0, w * fadeProgress, h);
        octx.clip();
      } else if (clip.transitionType === 'irisCircle') {
        const maxR = Math.hypot(w / 2, h / 2);
        octx.beginPath();
        octx.arc(w / 2, h / 2, maxR * fadeProgress, 0, Math.PI * 2);
        octx.clip();
      }
    }

    // Apply Pan/Crop Transform (including keyframe interpolation if present)
    const basePc = clip.panCrop || { x: 0, y: 0, scale: 1, rotation: 0, flipH: false, flipV: false };
    let pcX = basePc.x;
    let pcY = basePc.y;
    let pcScale = basePc.scale || 1;
    let pcRot = basePc.rotation || 0;

    if (basePc.keyframes && basePc.keyframes.length > 0) {
      const kfs = [...basePc.keyframes].sort((a, b) => a.time - b.time);
      if (localT <= kfs[0].time) {
        pcX = kfs[0].x;
        pcY = kfs[0].y;
        pcScale = kfs[0].scale;
        pcRot = kfs[0].rotation;
      } else if (localT >= kfs[kfs.length - 1].time) {
        const last = kfs[kfs.length - 1];
        pcX = last.x;
        pcY = last.y;
        pcScale = last.scale;
        pcRot = last.rotation;
      } else {
        for (let i = 0; i < kfs.length - 1; i++) {
          if (localT >= kfs[i].time && localT <= kfs[i + 1].time) {
            const span = Math.max(0.001, kfs[i + 1].time - kfs[i].time);
            const r = (localT - kfs[i].time) / span;
            pcX = kfs[i].x + (kfs[i + 1].x - kfs[i].x) * r;
            pcY = kfs[i].y + (kfs[i + 1].y - kfs[i].y) * r;
            pcScale = kfs[i].scale + (kfs[i + 1].scale - kfs[i].scale) * r;
            pcRot = kfs[i].rotation + (kfs[i + 1].rotation - kfs[i].rotation) * r;
            break;
          }
        }
      }
    }

    // Neoveg Pan/Crop works like a camera crop frame: smaller crop box = zoomed in!
    const zoomFactor = 1 / Math.max(0.2, Math.min(3.0, pcScale));
    octx.translate(w / 2 - pcX * (w * 0.45), h / 2 - pcY * (h * 0.45));
    if (pcRot) {
      octx.rotate((-pcRot * Math.PI) / 180);
    }
    const sx = zoomFactor * (basePc.flipH ? -1 : 1);
    const sy = zoomFactor * (basePc.flipV ? -1 : 1);
    octx.scale(sx, sy);
    octx.translate(-w / 2, -h / 2);

    // Build CSS filter from Clip FX + Track FX
    const allFx                    = [...(clip.fx || []), ...(track.fx || [])].filter((f) => f.enabled);
    const cssFilter = this.buildCssFilterString(allFx);

    if (!project.splitScreenBypass && cssFilter !== 'none') {
      octx.filter = cssFilter;
    }

    this.drawClipSource(octx, clip, localT, w, h);

    // If Split-Screen FX Bypass is active, render right half WITH FX and left half WITHOUT FX
    if (project.splitScreenBypass && cssFilter !== 'none') {
      octx.save();
      octx.beginPath();
      octx.rect(w / 2, 0, w / 2, h);
      octx.clip();
      octx.filter = cssFilter;
      this.drawClipSource(octx, clip, localT, w, h);
      octx.restore();
    }

    // Custom pixel/canvas overlays for Vignette, Color Grade Lift/Gain, VHS Scanlines
    for (const fx of allFx) {
      if (fx.type === 'vignette') {
        const strength = Number(fx.params.strength ?? 0.65);
        const grad = octx.createRadialGradient(w / 2, h / 2, w * 0.22, w / 2, h / 2, w * 0.62);
        grad.addColorStop(0, 'rgba(0,0,0,0)');
        grad.addColorStop(1, `rgba(0,0,0,${Math.min(0.95, strength)})`);
        octx.fillStyle = grad;
        octx.fillRect(0, 0, w, h);
      } else if (fx.type === 'colorGrade') {
        const liftR = Number(fx.params.liftR ?? 0);
        const liftB = Number(fx.params.liftB ?? 0);
        const gainR = Number(fx.params.gainR ?? 1);
        const gainB = Number(fx.params.gainB ?? 1);
        octx.save();
        octx.globalCompositeOperation = 'overlay';
        const rTint = Math.round(128 + (gainR - 1 + liftR) * 180);
        const bTint = Math.round(128 + (gainB - 1 + liftB) * 180);
        octx.fillStyle = `rgba(${Math.max(0, Math.min(255, rTint))}, 132, ${Math.max(0, Math.min(255, bTint))}, 0.28)`;
        octx.fillRect(0, 0, w, h);
        octx.restore();
      } else if (fx.type === 'vhsGlitch' || fx.type === 'frei0r.distort0r' || fx.type === 'frei0r.vertigo') {
        octx.save();
        octx.fillStyle = 'rgba(0, 0, 0, 0.18)';
        for (let y = 0; y < h; y += 4) {
          octx.fillRect(0, y, w, 1.5);
        }
        const barY = (localT * 90) % h;
        octx.fillStyle = 'rgba(59, 130, 246, 0.14)';
        octx.fillRect(0, barY, w, 14);
        octx.restore();
      } else if (fx.type === 'frei0r.glow') {
        octx.save();
        octx.globalCompositeOperation = 'screen';
        const gGrad = octx.createRadialGradient(w / 2, h / 2, 10, w / 2, h / 2, w * 0.55);
        gGrad.addColorStop(0, 'rgba(147, 197, 253, 0.28)');
        gGrad.addColorStop(1, 'rgba(30, 58, 138, 0.0)');
        octx.fillStyle = gGrad;
        octx.fillRect(0, 0, w, h);
        octx.restore();
      } else if (fx.type === 'corridorKey') {
        // Real-time Green Screen Color Unmixing & Spill Suppression (CorridorKey Engine)
        const imgData = octx.getImageData(0, 0, w, h);
        const d = imgData.data;
        const sim = Number(fx.params.similarity ?? 0.26) * 255;
        const despill = Number(fx.params.despill ?? 0.85);
        for (let i = 0; i < d.length; i += 4) {
          const r = d[i];
          const g = d[i + 1];
          const b = d[i + 2];
          const greenExcess = g - Math.max(r, b);
          if (greenExcess > 18) {
            const alpha = Math.max(0, Math.min(1, 1 - (greenExcess - 18) / Math.max(12, sim)));
            d[i + 3] = Math.round(d[i + 3] * alpha);
            // Color unmixing & spill removal on semi-transparent hair/edge pixels
            d[i + 1] = Math.round(g - greenExcess * despill);
          }
        }
        octx.putImageData(imgData, 0, 0);
      }
    }

    if (fadeProgress < 1.0 && clip.transitionType === 'flashWhite') {
      octx.fillStyle = `rgba(255,255,255,${(1 - fadeProgress) * 0.85})`;
      octx.fillRect(0, 0, w, h);
    }

    octx.restore();

    // Composite onto main canvas
    this.ctx.save();
    this.ctx.globalAlpha = effectiveAlpha;
    this.ctx.globalCompositeOperation = mapBlendMode(track.blendMode);
    this.ctx.drawImage(this.offscreen, 0, 0);

    if (project.splitScreenBypass) {
      this.ctx.strokeStyle = '#f97316';
      this.ctx.lineWidth = 2;
      this.ctx.beginPath();
      this.ctx.moveTo(w / 2, 0);
      this.ctx.lineTo(w / 2, h);
      this.ctx.stroke();
      this.ctx.fillStyle = 'rgba(0,0,0,0.65)';
      this.ctx.fillRect(8, 8, 86, 20);
      this.ctx.fillRect(w / 2 + 8, 8, 76, 20);
      this.ctx.fillStyle = '#e2e8f0';
      this.ctx.font = 'bold 10px Inter, sans-serif';
      this.ctx.fillText('BYPASS (RAW)', 14, 21);
      this.ctx.fillStyle = '#fb923c';
      this.ctx.fillText('FX ACTIVE', w / 2 + 14, 21);
    }
    this.ctx.restore();
  }

          buildCssFilterString(fxList                   )         {
    const parts           = [];
    for (const fx of fxList) {
      if (!fx.enabled) continue;
      if (fx.type === 'brightnessContrast') {
        const b = 100 + Number(fx.params.brightness ?? 0) * 100;
        const c = Number(fx.params.contrast ?? 1) * 100;
        const s = Number(fx.params.saturation ?? 1) * 100;
        parts.push(`brightness(${b.toFixed(0)}%) contrast(${c.toFixed(0)}%) saturate(${s.toFixed(0)}%)`);
      } else if (fx.type === 'colorGrade') {
        const sat = Number(fx.params.saturation ?? 1.15) * 100;
        parts.push(`saturate(${sat.toFixed(0)}%) contrast(106%)`);
      } else if (fx.type === 'blur') {
        const rad = Number(fx.params.radius ?? 4);
        parts.push(`blur(${rad.toFixed(1)}px)`);
      } else if (fx.type === 'invert') {
        parts.push('invert(100%)');
      } else if (fx.type === 'sepia') {
        const amt = Number(fx.params.amount ?? 0.85) * 100;
        parts.push(`sepia(${amt.toFixed(0)}%)`);
      } else if (fx.type === 'frei0r.cartoon' || fx.type === 'frei0r.posterize') {
        parts.push('contrast(145%) saturate(135%)');
      } else if (fx.type === 'frei0r.sobel') {
        parts.push('contrast(220%) invert(10%) saturate(180%)');
      } else if (fx.type === 'frei0r.coloradj_RGB') {
        const r = Number(fx.params.r ?? 1.12);
        parts.push(`brightness(${(r * 100).toFixed(0)}%) contrast(110%)`);
      }
    }
    return parts.length > 0 ? parts.join(' ') : 'none';
  }

          drawClipSource(
    ctx                          ,
    clip           ,
    localT        ,
    w        ,
    h        
  )       {
    // 1. Check if real HTMLVideoElement is ready
    if (clip.mediaUrl && clip.mediaType === 'video') {
      const vid = this.registerVideoElement(clip.mediaUrl);
      if (vid && vid.readyState >= 2) {
        ctx.drawImage(vid, 0, 0, w, h);
        // Subtle corner clip badge
        ctx.fillStyle = 'rgba(0,0,0,0.45)';
        ctx.fillRect(w - 150, h - 26, 142, 20);
        ctx.fillStyle = '#93c5fd';
        ctx.font = '10px JetBrains Mono, monospace';
        ctx.fillText(`${clip.name.slice(0, 18)}`, w - 144, h - 13);
        return;
      }
    }

    // 2. Procedural Neoveg Media Generators & Titles
    const gen = clip.generatorType || (clip.mediaType === 'title' ? 'titleText' : 'smpteBars');

    if (gen === 'titleText') {
      // Transparent lower-third / center title overlay
      const title = clip.titleText || clip.name || 'NEOVEG STUDIO 21';
      const sub = clip.subtitleText || 'NON-LINEAR VIDEO & AUDIO BROADCAST ENGINE';

      // Glassmorphic lower-third banner
      const bannerY = h * 0.68;
      const grad = ctx.createLinearGradient(w * 0.08, bannerY, w * 0.78, bannerY + 68);
      grad.addColorStop(0, 'rgba(15, 23, 42, 0.88)');
      grad.addColorStop(0.85, 'rgba(30, 58, 138, 0.72)');
      grad.addColorStop(1, 'rgba(30, 58, 138, 0.0)');
      ctx.fillStyle = grad;
      ctx.fillRect(w * 0.08, bannerY, w * 0.72, 66);

      // Accent bar
      ctx.fillStyle = clip.color || '#3b82f6';
      ctx.fillRect(w * 0.08, bannerY, 5, 66);

      ctx.fillStyle = '#f8fafc';
      ctx.font = `bold ${Math.round(h * 0.068)}px "Segoe UI", Inter, sans-serif`;
      ctx.shadowColor = 'rgba(0,0,0,0.75)';
      ctx.shadowBlur = 8;
      ctx.fillText(title, w * 0.1, bannerY + 32);

      ctx.fillStyle = '#93c5fd';
      ctx.font = `600 ${Math.round(h * 0.034)}px "JetBrains Mono", monospace`;
      ctx.fillText(sub, w * 0.1, bannerY + 54);
      ctx.shadowBlur = 0;
      return;
    }

    if (gen === 'creditRoll') {
      ctx.fillStyle = 'rgba(9, 11, 16, 0.85)';
      ctx.fillRect(0, 0, w, h);
      const scrollY = h - ((localT / Math.max(1, clip.duration)) * (h + 220));
      const lines = [
        'OPEN NEOVEG STUDIO',
        '────────────────────',
        'Core Timeline & DSP Engine: Rust (neoveg-core)',
        'Media Pipeline & Scripting API: Python + FFmpeg',
        'Studio NLE Interface: TypeScript + WebGL/Canvas',
        'Audio Engine: Multi-Bus Web Audio API',
        '5-Curve Auto-Crossfade Solver',
      ];
      ctx.textAlign = 'center';
      lines.forEach((line, idx) => {
        ctx.fillStyle = idx === 0 ? '#60a5fa' : '#e2e8f0';
        ctx.font = idx === 0 ? 'bold 20px Inter, sans-serif' : '13px Inter, sans-serif';
        ctx.fillText(line, w / 2, scrollY + idx * 32);
      });
      ctx.textAlign = 'left';
      return;
    }

    if (gen === 'cyberGrid' || clip.name.toLowerCase().includes('b-roll') || clip.name.toLowerCase().includes('cyber')) {
      // Dynamic Synthwave / Cyber Broadcast Scene
      const bgGrad = ctx.createLinearGradient(0, 0, 0, h);
      bgGrad.addColorStop(0, '#090d16');
      bgGrad.addColorStop(0.55, '#1e1b4b');
      bgGrad.addColorStop(0.56, '#0f172a');
      bgGrad.addColorStop(1, '#020617');
      ctx.fillStyle = bgGrad;
      ctx.fillRect(0, 0, w, h);

      // Sun
      const sunGrad = ctx.createLinearGradient(0, h * 0.15, 0, h * 0.55);
      sunGrad.addColorStop(0, '#f97316');
      sunGrad.addColorStop(1, '#ec4899');
      ctx.fillStyle = sunGrad;
      ctx.beginPath();
      ctx.arc(w / 2, h * 0.38, h * 0.18, 0, Math.PI * 2);
      ctx.fill();

      // Perspective horizon grid
      ctx.strokeStyle = 'rgba(56, 189, 248, 0.45)';
      ctx.lineWidth = 1.2;
      const horizon = h * 0.56;
      for (let i = -10; i <= 10; i++) {
        ctx.beginPath();
        ctx.moveTo(w / 2 + i * (w * 0.025), horizon);
        ctx.lineTo(w / 2 + i * (w * 0.14), h);
        ctx.stroke();
      }
      const speedOffset = (localT * 1.5) % 1;
      for (let row = 0; row < 8; row++) {
        const p = Math.pow((row + speedOffset) / 8, 1.8);
        const y = horizon + p * (h - horizon);
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(w, y);
        ctx.stroke();
      }

      ctx.fillStyle = '#38bdf8';
      ctx.font = 'bold 12px monospace';
      ctx.fillText(`SCENE: ${clip.name.toUpperCase()}  |  T+${localT.toFixed(2)}s`, 16, 26);
      return;
    }

    if (gen === 'solidColor') {
      ctx.fillStyle = clip.color || '#2563eb';
      ctx.fillRect(0, 0, w, h);
      return;
    }

    if (gen === 'greenScreenActor') {
      // CorridorKey Green Screen Studio Subject Demo (Actor silhouette + fine hair strands on #00e020 backdrop)
      ctx.fillStyle = '#00d820';
      ctx.fillRect(0, 0, w, h);

      const cx = w / 2 + Math.sin(localT * 1.4) * 18;
      const cy = h * 0.52;

      // Shoulders
      ctx.fillStyle = '#1e293b';
      ctx.beginPath();
      ctx.ellipse(cx, h + 10, w * 0.22, h * 0.28, 0, Math.PI, 0, false);
      ctx.fill();

      // Fine hair strands (for testing CorridorKey sub-pixel alpha unmixing)
      ctx.strokeStyle = 'rgba(42, 28, 22, 0.72)';
      ctx.lineWidth = 2;
      for (let a = -12; a <= 12; a++) {
        ctx.beginPath();
        ctx.moveTo(cx + a * 4, cy - h * 0.16);
        ctx.lineTo(cx + a * 5.8 + Math.sin(localT * 3 + a) * 3, cy - h * 0.24);
        ctx.stroke();
      }

      // Head
      ctx.fillStyle = '#e8b496';
      ctx.beginPath();
      ctx.ellipse(cx, cy - h * 0.04, w * 0.085, h * 0.17, 0, 0, Math.PI * 2);
      ctx.fill();

      // Eyes & smile
      ctx.fillStyle = '#0f172a';
      ctx.beginPath();
      ctx.arc(cx - w * 0.03, cy - h * 0.06, 4, 0, Math.PI * 2);
      ctx.arc(cx + w * 0.03, cy - h * 0.06, 4, 0, Math.PI * 2);
      ctx.fill();

      ctx.fillStyle = 'rgba(0,0,0,0.65)';
      ctx.fillRect(14, 14, 320, 24);
      ctx.fillStyle = '#4ade80';
      ctx.font = 'bold 11px monospace';
      ctx.fillText('CORRIDORKEY GREEN SCREEN SOURCE (ACTIVATE FX)', 22, 30);
      return;
    }

    // Default: Broadcast SMPTE Color Bars + Animated Radar Slate
    const bars = ['#c0c0c0', '#c0c000', '#00c0c0', '#00c000', '#c000c0', '#c00000', '#0000c0'];
    const barW = w / bars.length;
    bars.forEach((col, i) => {
      ctx.fillStyle = col;
      ctx.fillRect(i * barW, 0, barW + 1, h * 0.72);
    });
    ctx.fillStyle = '#0f172a';
    ctx.fillRect(0, h * 0.72, w, h * 0.28);

    // Animated sweep indicator
    const sweepX = ((localT * 160) % (w - 40)) + 20;
    ctx.fillStyle = '#3b82f6';
    ctx.fillRect(sweepX - 12, h * 0.76, 24, 8);

    ctx.fillStyle = '#f8fafc';
    ctx.font = 'bold 14px monospace';
    ctx.fillText(`Neoveg HD SMPTE TEST PATTERN — ${clip.name}`, 18, h * 0.87);
    ctx.fillStyle = '#94a3b8';
    ctx.font = '11px monospace';
    ctx.fillText(`TC: ${formatSMPTE(localT, 29.97, true)} | RATE: ${(clip.playbackRate || 1).toFixed(2)}x`, 18, h * 0.94);
  }

          drawGridOverlay(ctx                          , w        , h        )       {
    ctx.save();
    ctx.strokeStyle = 'rgba(148, 163, 184, 0.32)';
    ctx.lineWidth = 1;
    for (let i = 1; i <= 2; i++) {
      ctx.beginPath();
      ctx.moveTo((w * i) / 3, 0);
      ctx.lineTo((w * i) / 3, h);
      ctx.stroke();
      ctx.beginPath();
      ctx.moveTo(0, (h * i) / 3);
      ctx.lineTo(w, (h * i) / 3);
      ctx.stroke();
    }
    ctx.restore();
  }

          drawSafeAreasOverlay(ctx                          , w        , h        )       {
    ctx.save();
    ctx.strokeStyle = 'rgba(56, 189, 248, 0.45)';
    ctx.setLineDash([4, 4]);
    ctx.strokeRect(w * 0.05, h * 0.05, w * 0.9, h * 0.9); // 90% Action Safe
    ctx.strokeStyle = 'rgba(250, 204, 21, 0.45)';
    ctx.strokeRect(w * 0.1, h * 0.1, w * 0.8, h * 0.8); // 80% Title Safe
    ctx.setLineDash([]);
    ctx.restore();
  }

          drawPanCropGizmo(ctx                          , clip           , w        , h        )       {
    const pc = clip.panCrop || { x: 0, y: 0, scale: 1, rotation: 0, flipH: false, flipV: false };
    ctx.save();
    const cx = w / 2 + pc.x * (w * 0.5);
    const cy = h / 2 + pc.y * (h * 0.5);
    const boxW = w * 0.82 * (pc.scale || 1);
    const boxH = h * 0.82 * (pc.scale || 1);

    ctx.translate(cx, cy);
    ctx.rotate((pc.rotation * Math.PI) / 180);

    ctx.strokeStyle = '#38bdf8';
    ctx.lineWidth = 1.5;
    ctx.setLineDash([5, 3]);
    ctx.strokeRect(-boxW / 2, -boxH / 2, boxW, boxH);
    ctx.setLineDash([]);

    // Neoveg iconic "F" orientation watermark inside Pan/Crop box
    ctx.strokeStyle = 'rgba(56, 189, 248, 0.35)';
    ctx.lineWidth = 3;
    ctx.beginPath();
    ctx.moveTo(-14, 18);
    ctx.lineTo(-14, -18);
    ctx.lineTo(14, -18);
    ctx.moveTo(-14, 0);
    ctx.lineTo(8, 0);
    ctx.stroke();

    // Corner handles
    const corners = [
      [-boxW / 2, -boxH / 2],
      [boxW / 2, -boxH / 2],
      [boxW / 2, boxH / 2],
      [-boxW / 2, boxH / 2],
    ];
    ctx.fillStyle = '#38bdf8';
    for (const [hx, hy] of corners) {
      ctx.fillRect(hx - 4, hy - 4, 8, 8);
    }
    ctx.restore();
  }

         updateVideoScopes()       {
    if (!this.scopeCanvas) return;
    const sctx = this.scopeCanvas.getContext('2d');
    if (!sctx) return;

    const sw = this.scopeCanvas.width;
    const sh = this.scopeCanvas.height;
    sctx.fillStyle = '#080b11';
    sctx.fillRect(0, 0, sw, sh);

    const w = this.canvas.width;
    const h = this.canvas.height;
    if (w === 0 || h === 0) return;

    const img = this.ctx.getImageData(0, 0, w, h).data;
    const thirdW = sw / 3;

    // Horizontal IRE graticule lines (100, 75, 50, 25, 0 IRE) + vertical R | G | B dividers
    sctx.strokeStyle = 'rgba(148, 163, 184, 0.22)';
    sctx.lineWidth = 1;
    sctx.beginPath();
    for (const frac of [0.12, 0.34, 0.55, 0.76, 0.94]) {
      const gy = Math.round(sh * frac) + 0.5;
      sctx.moveTo(0, gy);
      sctx.lineTo(sw, gy);
    }
    sctx.moveTo(Math.round(thirdW) + 0.5, 0);
    sctx.lineTo(Math.round(thirdW) + 0.5, sh);
    sctx.moveTo(Math.round(thirdW * 2) + 0.5, 0);
    sctx.lineTo(Math.round(thirdW * 2) + 0.5, sh);
    sctx.stroke();

    // Channel badges R, G, B at top-left of each third
    sctx.font = 'bold 8px Consolas, monospace';
    sctx.fillStyle = '#f87171';
    sctx.fillText('R', 4, 9);
    sctx.fillStyle = '#4ade80';
    sctx.fillText('G', thirdW + 4, 9);
    sctx.fillStyle = '#60a5fa';
    sctx.fillText('B', thirdW * 2 + 4, 9);

    const stepX = Math.max(1, Math.floor(w / 54));
    const stepY = Math.max(1, Math.floor(h / 36));

    let maxLuma = 0;
    for (let x = 0; x < w; x += stepX) {
      const normX = x / w;
      for (let y = 0; y < h; y += stepY) {
        const idx = (y * w + x) * 4;
        const r = img[idx];
        const g = img[idx + 1];
        const b = img[idx + 2];
        if (r + g + b > maxLuma) maxLuma = r + g + b;

        const ry = sh - 5 - (r / 255) * (sh - 16);
        const gy = sh - 5 - (g / 255) * (sh - 16);
        const by = sh - 5 - (b / 255) * (sh - 16);

        sctx.fillStyle = 'rgba(248, 113, 113, 0.42)';
        sctx.fillRect(normX * (thirdW - 6) + 3, ry, 2, 2);

        sctx.fillStyle = 'rgba(74, 222, 128, 0.42)';
        sctx.fillRect(thirdW + normX * (thirdW - 6) + 3, gy, 2, 2);

        sctx.fillStyle = 'rgba(96, 165, 250, 0.42)';
        sctx.fillRect(thirdW * 2 + normX * (thirdW - 6) + 3, by, 2, 2);
      }
    }

    // If canvas is black/standby (empty timeline), draw subtle broadcast reference waveform so scope looks clean
    if (maxLuma < 15) {
      for (let i = 0; i < thirdW - 6; i += 2) {
        const phase = (i / (thirdW - 6)) * Math.PI * 2;
        const ry = sh * 0.56 + Math.sin(phase) * (sh * 0.22);
        const gy = sh * 0.54 + Math.cos(phase * 1.3) * (sh * 0.22);
        const by = sh * 0.58 + Math.sin(phase * 0.8 + 1.1) * (sh * 0.22);
        sctx.fillStyle = 'rgba(248, 113, 113, 0.55)';
        sctx.fillRect(3 + i, ry, 2, 2);
        sctx.fillStyle = 'rgba(74, 222, 128, 0.55)';
        sctx.fillRect(thirdW + 3 + i, gy, 2, 2);
        sctx.fillStyle = 'rgba(96, 165, 250, 0.55)';
        sctx.fillRect(thirdW * 2 + 3 + i, by, 2, 2);
      }
    }
  }
}


//# sourceURL=src/engine/compositor.ts

// ─── MODULE: src/engine/audioEngine.ts ───
/**
 * OpenNeoveg — Multi-Track Web Audio API Engine & Master Bus Meter
 */
class NeovegHostAudioEngine {
          audioElements                                = new Map();
          peakHoldL = -60;
          peakHoldR = -60;
          lastPeakTime = 0;

         registerAudioClip(clip           )                          {
    if (!clip.mediaUrl) return null;
    let el = this.audioElements.get(clip.id);
    if (!el) {
      el = new Audio(clip.mediaUrl);
      el.preload = 'auto';
      el.crossOrigin = 'anonymous';
      this.audioElements.set(clip.id, el);
    }
    return el;
  }

         syncAudio(project              , isPlaying         )                     {
    const t = project.playhead;
    const anySolo = project.tracks.some((tr) => tr.type === 'audio' && tr.solo);
    const masterGain = Math.max(0, Math.min(1.5, project.masterVolume ?? 1.0));

    let sumL = 0;
    let sumR = 0;
    const activeIds = new Set        ();

    for (const track of project.tracks) {
      if (track.type !== 'audio') continue;
      const trackActive = !track.muted && (!anySolo || track.solo);
      const trackVol = track.showEnvelope
        ? evaluateTrackEnvelope(track.envelopePoints, t, track.level)
        : track.level;
      const pan = Math.max(-1, Math.min(1, track.pan || 0));
      const panL = Math.cos(((pan + 1) * Math.PI) / 4);
      const panR = Math.sin(((pan + 1) * Math.PI) / 4);

      for (const clip of track.clips) {
        activeIds.add(clip.id);
        const inRange = t >= clip.start && t <= clip.start + clip.duration;
        const fadeMult = computeClipFadeMultiplier(clip, t);
        const effectiveGain = trackActive && !clip.muted && inRange
          ? Math.max(0, Math.min(1.5, fadeMult * trackVol * masterGain))
          : 0;

        // Sync underlying HTMLAudioElement if URL is present
        if (clip.mediaUrl) {
          const el = this.registerAudioClip(clip);
          if (el) {
            el.volume = Math.max(0, Math.min(1, effectiveGain));
            el.playbackRate = Math.max(0.25, Math.min(4.0, clip.playbackRate || 1.0));
            if (isPlaying && effectiveGain > 0.001) {
              const targetT = clip.trimIn + (t - clip.start) * (clip.playbackRate || 1);
              const dur = el.duration || 8;
              const wrapped = targetT % dur;
              if (Math.abs(el.currentTime - wrapped) > 0.22) {
                el.currentTime = wrapped;
              }
              if (el.paused) {
                el.play().catch(() => {});
              }
            } else if (!el.paused) {
              el.pause();
            }
          }
        }

        // Compute instantaneous waveform amplitude for accurate dB metering
        if (effectiveGain > 0.001 && inRange) {
          const localRatio = Math.max(0, Math.min(0.999, (t - clip.start) / Math.max(0.05, clip.duration)));
          let sampleAmp = 0.55;
          if (clip.peaks && clip.peaks.length > 0) {
            const idx = Math.floor(localRatio * clip.peaks.length);
            sampleAmp = clip.peaks[idx] ?? 0.5;
          } else {
            sampleAmp = 0.45 + 0.35 * Math.abs(Math.sin(t * 14 + clip.start));
          }
          // Add micro transient modulation while playing
          const mod = isPlaying ? 0.88 + 0.22 * Math.abs(Math.sin(t * 37.0)) : 1.0;
          const level = sampleAmp * effectiveGain * mod;
          sumL += level * panL;
          sumR += level * panR;
        }
      }
    }

    // Pause any orphaned audio elements
    for (const [id, el] of this.audioElements.entries()) {
      if ((!isPlaying || !activeIds.has(id)) && !el.paused) {
        el.pause();
      }
    }

    const clampL = Math.max(0, Math.min(1.25, sumL));
    const clampR = Math.max(0, Math.min(1.25, sumR));
    const leftDb = clampL > 0.001 ? 20 * Math.log10(clampL) : -60;
    const rightDb = clampR > 0.001 ? 20 * Math.log10(clampR) : -60;

    const now = performance.now();
    if (leftDb > this.peakHoldL || now - this.lastPeakTime > 1800) {
      this.peakHoldL = leftDb;
      this.lastPeakTime = now;
    }
    if (rightDb > this.peakHoldR || now - this.lastPeakTime > 1800) {
      this.peakHoldR = rightDb;
      this.lastPeakTime = now;
    }

    return {
      leftLinear: clampL,
      rightLinear: clampR,
      leftDb: Math.max(-60, Math.min(6, leftDb)),
      rightDb: Math.max(-60, Math.min(6, rightDb)),
      peakLeftDb: Math.max(-60, Math.min(6, this.peakHoldL)),
      peakRightDb: Math.max(-60, Math.min(6, this.peakHoldR)),
      clipping: leftDb > 0.1 || rightDb > 0.1,
    };
  }

         stopAll()       {
    for (const el of this.audioElements.values()) {
      if (!el.paused) el.pause();
    }
  }
}


//# sourceURL=src/engine/audioEngine.ts

// ─── MODULE: src/ui/timeline.ts ───
/**
 * OpenNeoveg — Interactive Multi-Track Timeline Canvas & Event Manipulation Engine
 * Implements Neoveg's signature editing paradigm:
 * - Individual & Global Track Height Resizing (Drag track bottom border, +/- buttons, or Alt+Wheel)
 * - Direct Media / Generator / FX / Transition / OS File Drag-and-Drop onto Timeline with Ghost Preview
 * - Automatic Overlap Crossfades (X-curve)
 * - Top-corner interactive Fade-In / Fade-Out handles with 5 Neoveg curve shapes
 * - Clip Opacity / Gain horizontal envelope bar
 * - Edge Trimming, Snap to Playhead/Events/Markers (F8), Auto-Ripple (Ctrl+L)
 * - On-clip FX & Pan/Crop badges, Track Automation Envelopes (V), Markers (M) & Loop Region (Q)
 */
const RULER_HEIGHT = 38;
const DEFAULT_TRACK_HEIGHT = 76;
const MIN_TRACK_HEIGHT = 44;
const MAX_TRACK_HEIGHT = 220;
class NeovegHostTimelineController {
          canvas                   ;
          ctx                          ;
          project              ;
          callbacks                   ;

         pixelsPerSecond = 88;
         scrollX = 0;
         selectedClipId                = null;
         selectedTrackId                = null;
         editTool               = 'normal';
         lightThemeCanvas = false;

          dragMode           = 'none';
          dragClip                   = null;
          dragTrack               = null;
          dragEnvelopeIdx = -1;
          dragStartX = 0;
          dragStartY = 0;
          origStart = 0;
          origDuration = 0;
          origTrimIn = 0;
          origFadeIn = 0;
          origFadeOut = 0;
          origGain = 1;
          origTrackHeight = DEFAULT_TRACK_HEIGHT;
          hoverStatus = 'Ready';

  // Live Ghost Drop Preview when dragging media/FX/files over the timeline
          dropPreview   
                    
                 
                     
                     
                  
                  
           = null;

  constructor(
    canvas                   ,
    project              ,
    callbacks                   
  ) {
    this.canvas = canvas;
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('Failed to initialize timeline 2D context');
    this.ctx = ctx;
    this.project = project;
    this.callbacks = callbacks;

    this.bindEvents();
  }

         setProject(project              )       {
    this.project = project;
    this.recalculateAutoCrossfades();
    this.render();
  }

         getTrackHeight(track       )         {
    return Math.max(MIN_TRACK_HEIGHT, Math.min(MAX_TRACK_HEIGHT, Math.round(track.height || DEFAULT_TRACK_HEIGHT)));
  }

         getTrackTop(trackIdx        )         {
    let y = RULER_HEIGHT;
    for (let i = 0; i < trackIdx && i < this.project.tracks.length; i++) {
      y += this.getTrackHeight(this.project.tracks[i]);
    }
    return y;
  }

         getTotalTracksHeight()         {
    let sum = 0;
    for (const tr of this.project.tracks) {
      sum += this.getTrackHeight(tr);
    }
    return sum;
  }

         findTrackAtY(y        )   
                 
                     
                     
                   
                              
           {
    if (y <= RULER_HEIGHT) return null;
    let curTop = RULER_HEIGHT;
    for (let i = 0; i < this.project.tracks.length; i++) {
      const tr = this.project.tracks[i];
      const h = this.getTrackHeight(tr);
      const bottom = curTop + h;
      if (y >= curTop && y <= bottom + 4) {
        const nearBottomBorder = Math.abs(y - bottom) <= 5;
        return { track: tr, trackIdx: i, trackTop: curTop, trackH: h, nearBottomBorder };
      }
      curTop = bottom;
    }
    return null;
  }

         adjustAllTrackHeights(deltaPx        , filterType                    )       {
    for (const tr of this.project.tracks) {
      if (filterType && tr.type !== filterType) continue;
      tr.height = Math.max(
        MIN_TRACK_HEIGHT,
        Math.min(MAX_TRACK_HEIGHT, this.getTrackHeight(tr) + deltaPx)
      );
    }
    this.callbacks.onTrackHeightChanged?.();
    this.render();
  }

         adjustSingleTrackHeight(trackId        , deltaPx        )       {
    const tr = this.project.tracks.find((t) => t.id === trackId);
    if (!tr) return;
    tr.height = Math.max(
      MIN_TRACK_HEIGHT,
      Math.min(MAX_TRACK_HEIGHT, this.getTrackHeight(tr) + deltaPx)
    );
    this.callbacks.onTrackHeightChanged?.();
    this.render();
  }

         timeToX(sec        )         {
    return (sec - this.scrollX) * this.pixelsPerSecond;
  }

         xToTime(x        )         {
    return Math.max(0, this.scrollX + x / this.pixelsPerSecond);
  }

         snapTime(targetSec        , excludeClipId         )         {
    if (!this.project.snapEnabled) return Math.max(0, targetSec);
    const thresholdSec = 8 / this.pixelsPerSecond;
    const candidates           = [
      0,
      this.project.playhead,
      this.project.loopStart,
      this.project.loopEnd,
    ];
    for (const m of this.project.markers) candidates.push(m.time);
    for (const r of this.project.regions) {
      candidates.push(r.start, r.end);
    }
    for (const tr of this.project.tracks) {
      for (const c of tr.clips) {
        if (c.id === excludeClipId) continue;
        candidates.push(c.start, c.start + c.duration);
      }
    }
    const nearestGrid = Math.round(targetSec * 2) / 2;
    candidates.push(nearestGrid);

    let best = targetSec;
    let minDiff = thresholdSec;
    for (const cand of candidates) {
      const diff = Math.abs(cand - targetSec);
      if (diff < minDiff) {
        minDiff = diff;
        best = cand;
      }
    }
    return Math.max(0, Number(best.toFixed(3)));
  }

         recalculateAutoCrossfades()                     {
    const overlaps                     = [];
    if (!this.project.autoCrossfadeEnabled) return overlaps;

    this.project.tracks.forEach((track, tIdx) => {
      track.clips.sort((a, b) => a.start - b.start);
      for (let i = 0; i < track.clips.length; i++) {
        for (let j = i + 1; j < track.clips.length; j++) {
          const outClip = track.clips[i];
          const inClip = track.clips[j];
          const outEnd = outClip.start + outClip.duration;
          if (inClip.start < outEnd - 0.008) {
            const overlapEnd = Math.min(outEnd, inClip.start + inClip.duration);
            const overlapDur = Math.max(0, overlapEnd - inClip.start);
            if (overlapDur > 0.01) {
              outClip.fadeOut = Number(overlapDur.toFixed(3));
              inClip.fadeIn = Number(overlapDur.toFixed(3));
              overlaps.push({
                trackId: track.id,
                trackIndex: tIdx,
                outgoingClipId: outClip.id,
                incomingClipId: inClip.id,
                start: inClip.start,
                end: overlapEnd,
                duration: overlapDur,
                outCurve: outClip.fadeOutCurve,
                inCurve: inClip.fadeInCurve,
              });
            }
          }
        }
      }
    });
    return overlaps;
  }

          bindEvents()       {
    window.addEventListener('resize', () => this.render());

    this.canvas.addEventListener(
      'wheel',
      (e) => {
        e.preventDefault();
        if (e.altKey) {
          // Alt + Wheel: Increase / Decrease Track Heights vertically
          const delta = e.deltaY < 0 ? 10 : -10;
          this.adjustAllTrackHeights(delta);
          return;
        }
        if (e.ctrlKey || e.metaKey || Math.abs(e.deltaY) > 0) {
          if (e.shiftKey) {
            this.scrollX = Math.max(0, this.scrollX + (e.deltaY > 0 ? 0.8 : -0.8));
          } else {
            const mouseTime = this.xToTime(e.offsetX);
            const factor = e.deltaY < 0 ? 1.15 : 0.87;
            this.pixelsPerSecond = Math.max(24, Math.min(360, this.pixelsPerSecond * factor));
            this.scrollX = Math.max(0, mouseTime - e.offsetX / this.pixelsPerSecond);
          }
          this.render();
        }
      },
      { passive: false }
    );

    this.canvas.addEventListener('mousedown', (e) => this.handleMouseDown(e));
    window.addEventListener('mousemove', (e) => this.handleMouseMove(e));
    window.addEventListener('mouseup', () => this.handleMouseUp());

    // Direct Drag-and-Drop Media / Generators / FX / Files onto Timeline Canvas
    this.canvas.addEventListener('dragover', (e) => {
      e.preventDefault();
      if (e.dataTransfer) {
        e.dataTransfer.dropEffect = 'copy';
      }
      const rect = this.canvas.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      const dropTime = this.snapTime(this.xToTime(x));
      const found = this.findTrackAtY(y);
      const trackIdx = found ? found.trackIdx : Math.max(0, this.project.tracks.length - 1);

      const payload = this.callbacks.getActiveDragPayload?.();
      const label = payload ? payload.name : 'Soltar Mídia / Efeito Aqui';
      const dur = payload?.duration || 5.0;
      const color = payload?.color || '#38bdf8';

      this.dropPreview = {
        active: true,
        time: dropTime,
        duration: dur,
        trackIdx,
        label,
        color,
      };
      this.render();
    });

    this.canvas.addEventListener('dragleave', () => {
      if (this.dropPreview) {
        this.dropPreview = null;
        this.render();
      }
    });

    this.canvas.addEventListener('drop', (e) => {
      e.preventDefault();
      const rect = this.canvas.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      const dropTime = this.snapTime(this.xToTime(x));
      const found = this.findTrackAtY(y);
      this.dropPreview = null;
      this.callbacks.onTimelineDrop?.(dropTime, found ? found.track : null, e);
      this.render();
    });

    this.canvas.addEventListener('dblclick', (e) => {
      const rect = this.canvas.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      const found = this.findTrackAtY(y);
      if (found && found.track.showEnvelope) {
        const relY = y - found.trackTop;
        const val = Math.max(0, Math.min(1, 1 - (relY - 12) / (found.trackH - 24)));
        found.track.envelopePoints.push({
          time: Number(this.xToTime(x).toFixed(3)),
          value: Number(val.toFixed(3)),
        });
        found.track.envelopePoints.sort((a, b) => a.time - b.time);
        this.callbacks.onProjectMutated('Added automation envelope point');
        this.render();
      }
    });

    this.canvas.addEventListener('contextmenu', (e) => {
      e.preventDefault();
      const rect = this.canvas.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      const hit = this.hitTestClip(x, y);
      if (hit) {
        this.selectedClipId = hit.clip.id;
        this.selectedTrackId = hit.track.id;
        this.callbacks.onSelectClip(hit.clip, hit.track);
        this.render();
        this.callbacks.onContextMenuClip(hit.clip, hit.track, e.clientX, e.clientY);
      }
    });
  }

          hitTestClip(
    x        ,
    y        
  )   
                    
                 
                     
                  
                  
                     
                   
           {
    const found = this.findTrackAtY(y);
    if (!found) return null;
    const { track, trackIdx, trackTop, trackH } = found;

    for (let i = track.clips.length - 1; i >= 0; i--) {
      const clip = track.clips[i];
      const cx = this.timeToX(clip.start);
      const cw = Math.max(12, clip.duration * this.pixelsPerSecond);
      if (x >= cx && x <= cx + cw && y >= trackTop + 4 && y <= trackTop + trackH - 4) {
        return { clip, track, trackIdx, clipX: cx, clipW: cw, trackTop, trackH };
      }
    }
    return null;
  }

          handleMouseDown(e            )       {
    if (e.button !== 0) return;
    const rect = this.canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    this.dragStartX = x;
    this.dragStartY = y;

    // 1. Check Ruler interaction (Loop handles or Playhead scrub)
    if (y <= RULER_HEIGHT) {
      const loopStartX = this.timeToX(this.project.loopStart);
      const loopEndX = this.timeToX(this.project.loopEnd);
      if (y <= 16 && Math.abs(x - loopStartX) <= 8) {
        this.dragMode = 'loopStart';
        return;
      }
      if (y <= 16 && Math.abs(x - loopEndX) <= 8) {
        this.dragMode = 'loopEnd';
        return;
      }
      this.dragMode = 'scrub';
      this.callbacks.onPlayheadChange(this.xToTime(x));
      this.render();
      return;
    }

    // 2. Check Track Bottom Divider Resize or Track Envelope Point
    const found = this.findTrackAtY(y);
    if (found) {
      const { track, trackTop, trackH, nearBottomBorder } = found;
      this.selectedTrackId = track.id;

      if (nearBottomBorder) {
        this.dragMode = 'resizeTrackHeight';
        this.dragTrack = track;
        this.origTrackHeight = trackH;
        return;
      }

      if (track.showEnvelope) {
        for (let i = 0; i < track.envelopePoints.length; i++) {
          const pt = track.envelopePoints[i];
          const px = this.timeToX(pt.time);
          const py = trackTop + 12 + (1 - pt.value) * (trackH - 24);
          if (Math.hypot(x - px, y - py) <= 8) {
            this.dragMode = 'envelopePoint';
            this.dragTrack = track;
            this.dragEnvelopeIdx = i;
            return;
          }
        }
      }
    }

    // 3. Check Clip hit test
    const hit = this.hitTestClip(x, y);
    if (!hit) {
      this.selectedClipId = null;
      this.callbacks.onSelectClip(null, found?.track || null);
      this.dragMode = 'scrub';
      this.callbacks.onPlayheadChange(this.xToTime(x));
      this.render();
      return;
    }

    const { clip, clipX, clipW, trackTop, trackH } = hit;
    this.selectedClipId = clip.id;
    this.selectedTrackId = hit.track.id;
    this.callbacks.onSelectClip(clip, hit.track);

    if (hit.track.locked || clip.locked) {
      this.render();
      return;
    }

    // Check On-Clip Mini Badges (Top Right of clip: Pan/Crop & Event FX buttons)
    const relY = y - (trackTop + 4);
    if (relY <= 18 && clipW > 70) {
      const fxBtnX = clipX + clipW - 22;
      const cropBtnX = clipX + clipW - 42;
      if (x >= fxBtnX && x <= fxBtnX + 18) {
        this.callbacks.onOpenClipFX(clip, hit.track);
        return;
      }
      if (hit.track.type === 'video' && x >= cropBtnX && x <= cropBtnX + 18) {
        this.callbacks.onOpenClipPanCrop(clip, hit.track);
        return;
      }
    }

    // If Razor Tool is active, split immediately at click point
    if (this.editTool === 'razor') {
      const clickTime = this.snapTime(this.xToTime(x));
      this.callbacks.onRazorSplitAtTime?.(clip, hit.track, clickTime);
      return;
    }

    this.dragClip = clip;
    this.dragTrack = hit.track;
    this.origStart = clip.start;
    this.origDuration = clip.duration;
    this.origTrimIn = clip.trimIn;
    this.origFadeIn = clip.fadeIn;
    this.origFadeOut = clip.fadeOut;
    this.origGain = clip.gain;

    if (this.editTool === 'slip') {
      this.dragMode = 'slipClip';
      this.render();
      return;
    }
    if (this.editTool === 'rateStretch' || e.ctrlKey) {
      this.dragMode = 'rateStretch';
      this.render();
      return;
    }

    const fadeInHandleX = clipX + clip.fadeIn * this.pixelsPerSecond;
    const fadeOutHandleX = clipX + clipW - clip.fadeOut * this.pixelsPerSecond;

    if (relY <= 18 && Math.abs(x - fadeInHandleX) <= 10) {
      this.dragMode = 'fadeIn';
    } else if (relY <= 18 && Math.abs(x - fadeOutHandleX) <= 10) {
      this.dragMode = 'fadeOut';
    } else if (Math.abs(x - clipX) <= 7) {
      this.dragMode = 'trimLeft';
    } else if (Math.abs(x - (clipX + clipW)) <= 7) {
      this.dragMode = 'trimRight';
    } else {
      const gainNorm = Math.max(0, Math.min(1, clip.gain));
      const gainLineY = trackTop + 22 + (1 - gainNorm) * (trackH - 30);
      if (Math.abs(y - gainLineY) <= 5 && relY > 18) {
        this.dragMode = 'clipGain';
      } else {
        this.dragMode = 'moveClip';
      }
    }

    this.render();
  }

          handleMouseMove(e            )       {
    const rect = this.canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    if (this.dragMode === 'none') {
      this.updateHoverCursor(x, y);
      return;
    }

    if (this.dragMode === 'resizeTrackHeight' && this.dragTrack) {
      const dy = y - this.dragStartY;
      this.dragTrack.height = Math.max(
        MIN_TRACK_HEIGHT,
        Math.min(MAX_TRACK_HEIGHT, Math.round(this.origTrackHeight + dy))
      );
      this.callbacks.onTrackHeightChanged?.();
      this.render();
      return;
    }

    const dxSec = (x - this.dragStartX) / this.pixelsPerSecond;

    if (this.dragMode === 'scrub') {
      this.callbacks.onPlayheadChange(this.xToTime(x));
      this.render();
      return;
    }

    if (this.dragMode === 'loopStart') {
      this.project.loopStart = Math.max(0, Math.min(this.project.loopEnd - 0.2, this.snapTime(this.xToTime(x))));
      this.render();
      return;
    }

    if (this.dragMode === 'loopEnd') {
      this.project.loopEnd = Math.max(this.project.loopStart + 0.2, this.snapTime(this.xToTime(x)));
      this.render();
      return;
    }

    if (this.dragMode === 'envelopePoint' && this.dragTrack) {
      const tIdx = this.project.tracks.indexOf(this.dragTrack);
      const trackTop = this.getTrackTop(tIdx);
      const trackH = this.getTrackHeight(this.dragTrack);
      const pt = this.dragTrack.envelopePoints[this.dragEnvelopeIdx];
      if (pt) {
        pt.time = Number(Math.max(0, this.xToTime(x)).toFixed(3));
        pt.value = Number(Math.max(0, Math.min(1, 1 - (y - trackTop - 12) / (trackH - 24))).toFixed(3));
        this.render();
      }
      return;
    }

    if (!this.dragClip || !this.dragTrack) return;
    const clip = this.dragClip;

    if (this.dragMode === 'moveClip') {
      const rawStart = Math.max(0, this.origStart + dxSec);
      const snappedStart = this.snapTime(rawStart, clip.id);
      const deltaStart = snappedStart - clip.start;
      clip.start = snappedStart;

      if (this.project.rippleEnabled && Math.abs(deltaStart) > 0.0001) {
        for (const other of this.dragTrack.clips) {
          if (other.id !== clip.id && other.start > this.origStart) {
            other.start = Math.max(0, Number((other.start + deltaStart).toFixed(3)));
          }
        }
      }

      const foundTarget = this.findTrackAtY(y);
      if (foundTarget) {
        const targetTrack = foundTarget.track;
        if (
          targetTrack &&
          targetTrack.id !== this.dragTrack.id &&
          targetTrack.type === this.dragTrack.type &&
          !targetTrack.locked
        ) {
          this.dragTrack.clips = this.dragTrack.clips.filter((c) => c.id !== clip.id);
          targetTrack.clips.push(clip);
          this.dragTrack = targetTrack;
          this.selectedTrackId = targetTrack.id;
        }
      }

      this.recalculateAutoCrossfades();
      this.render();
    } else if (this.dragMode === 'trimLeft') {
      const maxDelta = this.origDuration - 0.15;
      const clampedDelta = Math.max(-this.origStart, Math.min(maxDelta, dxSec));
      const newStart = this.snapTime(this.origStart + clampedDelta, clip.id);
      const realDelta = newStart - this.origStart;
      clip.start = Number(newStart.toFixed(3));
      clip.duration = Number(Math.max(0.15, this.origDuration - realDelta).toFixed(3));
      clip.trimIn = Number(Math.max(0, this.origTrimIn + realDelta).toFixed(3));
      this.recalculateAutoCrossfades();
      this.render();
    } else if (this.dragMode === 'trimRight') {
      const rawEnd = Math.max(clip.start + 0.15, this.origStart + this.origDuration + dxSec);
      const snappedEnd = this.snapTime(rawEnd, clip.id);
      clip.duration = Number(Math.max(0.15, snappedEnd - clip.start).toFixed(3));
      this.recalculateAutoCrossfades();
      this.render();
    } else if (this.dragMode === 'fadeIn') {
      clip.fadeIn = Number(Math.max(0, Math.min(clip.duration - 0.05, this.origFadeIn + dxSec)).toFixed(3));
      this.render();
    } else if (this.dragMode === 'fadeOut') {
      clip.fadeOut = Number(Math.max(0, Math.min(clip.duration - 0.05, this.origFadeOut - dxSec)).toFixed(3));
      this.render();
    } else if (this.dragMode === 'clipGain') {
      const dy = this.dragStartY - y;
      clip.gain = Number(Math.max(0, Math.min(1.5, this.origGain + dy / 50)).toFixed(2));
      this.render();
    } else if (this.dragMode === 'slipClip') {
      clip.trimIn = Number(Math.max(0, this.origTrimIn - dxSec).toFixed(3));
      this.render();
    } else if (this.dragMode === 'rateStretch') {
      const newDur = Math.max(0.25, this.origDuration + dxSec);
      const ratio = this.origDuration / newDur;
      clip.duration = Number(newDur.toFixed(3));
      clip.playbackRate = Number(Math.max(0.25, Math.min(4.0, ratio)).toFixed(2));
      this.recalculateAutoCrossfades();
      this.render();
    }
  }

          handleMouseUp()       {
    if (this.dragMode !== 'none') {
      const mode = this.dragMode;
      this.dragMode = 'none';
      this.dragClip = null;
      this.dragTrack = null;
      if (mode !== 'scrub' && mode !== 'resizeTrackHeight') {
        this.recalculateAutoCrossfades();
        this.callbacks.onProjectMutated(`Timeline edit (${mode})`);
      }
      this.render();
    }
  }

          updateHoverCursor(x        , y        )       {
    if (y <= RULER_HEIGHT) {
      this.canvas.style.cursor = 'pointer';
      return;
    }
    const found = this.findTrackAtY(y);
    if (found && found.nearBottomBorder) {
      this.canvas.style.cursor = 'ns-resize';
      this.hoverStatus = 'Arraste para aumentar/diminuir a altura da faixa';
      return;
    }

    const hit = this.hitTestClip(x, y);
    if (!hit) {
      this.canvas.style.cursor = 'default';
      return;
    }
    const { clip, clipX, clipW, trackTop, trackH } = hit;
    const relY = y - (trackTop + 4);
    const fadeInX = clipX + clip.fadeIn * this.pixelsPerSecond;
    const fadeOutX = clipX + clipW - clip.fadeOut * this.pixelsPerSecond;

    if (relY <= 18 && (Math.abs(x - fadeInX) <= 10 || Math.abs(x - fadeOutX) <= 10)) {
      this.canvas.style.cursor = 'ew-resize';
      this.hoverStatus = 'Drag corner to adjust Neoveg Fade-In / Fade-Out envelope';
    } else if (Math.abs(x - clipX) <= 7 || Math.abs(x - (clipX + clipW)) <= 7) {
      this.canvas.style.cursor = 'col-resize';
      this.hoverStatus = 'Drag edge to Trim event start/end';
    } else {
      const gainNorm = Math.max(0, Math.min(1, clip.gain));
      const gainLineY = trackTop + 22 + (1 - gainNorm) * (trackH - 30);
      if (Math.abs(y - gainLineY) <= 5 && relY > 18) {
        this.canvas.style.cursor = 'ns-resize';
      } else {
        this.canvas.style.cursor = 'grab';
      }
    }
  }

         render()       {
    const parent = this.canvas.parentElement;
    if (parent) {
      const dpr = window.devicePixelRatio || 1;
      const targetW = parent.clientWidth;
      const targetH = Math.max(
        parent.clientHeight,
        RULER_HEIGHT + this.getTotalTracksHeight() + 24
      );
      if (this.canvas.width !== Math.floor(targetW * dpr) || this.canvas.height !== Math.floor(targetH * dpr)) {
        this.canvas.width = Math.floor(targetW * dpr);
        this.canvas.height = Math.floor(targetH * dpr);
        this.canvas.style.width = `${targetW}px`;
        this.canvas.style.height = `${targetH}px`;
      }
      this.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    }

    const w = this.canvas.clientWidth || 900;
    const h = this.canvas.clientHeight || 360;
    const ctx = this.ctx;

    // 1. Background
    ctx.fillStyle = this.lightThemeCanvas ? '#d8d5ce' : '#14161b';
    ctx.fillRect(0, 0, w, h);

    // 2. Vertical Time Grid Lines
    this.drawTimeGrid(ctx, w, h);

    // 3. Track Lanes & Clips (with dynamic per-track height!)
    let curTop = RULER_HEIGHT;
    this.project.tracks.forEach((track, idx) => {
      const trackH = this.getTrackHeight(track);
      this.drawTrackRow(ctx, track, idx, curTop, trackH, w);
      curTop += trackH;
    });

    // 4. Ghost Drag-and-Drop Preview Box
    if (this.dropPreview && this.dropPreview.active) {
      const dp = this.dropPreview;
      const trIdx = Math.max(0, Math.min(this.project.tracks.length - 1, dp.trackIdx));
      const tr = this.project.tracks[trIdx];
      const top = this.getTrackTop(trIdx);
      const trH = tr ? this.getTrackHeight(tr) : DEFAULT_TRACK_HEIGHT;
      const gx = this.timeToX(dp.time);
      const gw = Math.max(60, dp.duration * this.pixelsPerSecond);

      ctx.save();
      ctx.fillStyle = 'rgba(56, 189, 248, 0.24)';
      ctx.fillRect(gx, top + 4, gw, trH - 8);
      ctx.strokeStyle = '#38bdf8';
      ctx.lineWidth = 2;
      ctx.setLineDash([5, 3]);
      ctx.strokeRect(gx, top + 4, gw, trH - 8);
      ctx.setLineDash([]);
      ctx.fillStyle = '#ffffff';
      ctx.font = 'bold 11px "Segoe UI", sans-serif';
      ctx.fillText(`+ ${dp.label} [${formatSMPTE(dp.time, this.project.fps)}]`, gx + 6, top + 20);
      ctx.restore();
    }

    // 5. Loop Region Overlay across tracks
    const loopX1 = this.timeToX(this.project.loopStart);
    const loopX2 = this.timeToX(this.project.loopEnd);
    if (loopX2 > loopX1) {
      ctx.fillStyle = this.project.loopEnabled
        ? 'rgba(56, 189, 248, 0.07)'
        : 'rgba(148, 163, 184, 0.03)';
      ctx.fillRect(loopX1, RULER_HEIGHT, loopX2 - loopX1, h - RULER_HEIGHT);
      ctx.strokeStyle = this.project.loopEnabled ? 'rgba(56, 189, 248, 0.45)' : 'rgba(148, 163, 184, 0.25)';
      ctx.lineWidth = 1;
      ctx.setLineDash([4, 4]);
      ctx.beginPath();
      ctx.moveTo(loopX1, RULER_HEIGHT);
      ctx.lineTo(loopX1, h);
      ctx.moveTo(loopX2, RULER_HEIGHT);
      ctx.lineTo(loopX2, h);
      ctx.stroke();
      ctx.setLineDash([]);
    }

    // 6. Top Timecode Ruler, Loop Bar, Regions & Markers
    this.drawRuler(ctx, w, h);

    // 7. Playhead Cursor
    const playX = this.timeToX(this.project.playhead);
    if (playX >= -10 && playX <= w + 10) {
      ctx.save();
      ctx.strokeStyle = '#f8fafc';
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.moveTo(playX, 12);
      ctx.lineTo(playX, h);
      ctx.stroke();

      ctx.fillStyle = '#38bdf8';
      ctx.beginPath();
      ctx.moveTo(playX - 7, 14);
      ctx.lineTo(playX + 7, 14);
      ctx.lineTo(playX, 25);
      ctx.closePath();
      ctx.fill();
      ctx.restore();
    }
  }

          drawTimeGrid(ctx                          , w        , h        )       {
    const stepSec = this.pixelsPerSecond > 140 ? 0.5 : this.pixelsPerSecond > 65 ? 1 : 2;
    const firstSec = Math.floor(this.scrollX / stepSec) * stepSec;
    const endSec = this.xToTime(w);

    ctx.lineWidth = 1;
    for (let s = firstSec; s <= endSec; s += stepSec) {
      const x = Math.round(this.timeToX(s)) + 0.5;
      if (x < 0) continue;
      const isMajor = Math.abs(s % (stepSec * 2)) < 0.001;
      ctx.strokeStyle = isMajor ? 'rgba(255,255,255,0.07)' : 'rgba(255,255,255,0.03)';
      ctx.beginPath();
      ctx.moveTo(x, RULER_HEIGHT);
      ctx.lineTo(x, h);
      ctx.stroke();
    }
  }

          drawRuler(ctx                          , w        , h        )       {
    ctx.fillStyle = this.lightThemeCanvas ? '#c6c3bc' : '#1d2028';
    ctx.fillRect(0, 0, w, RULER_HEIGHT);
    ctx.strokeStyle = this.lightThemeCanvas ? '#8c8982' : '#2e3340';
    ctx.beginPath();
    ctx.moveTo(0, RULER_HEIGHT - 0.5);
    ctx.lineTo(w, RULER_HEIGHT - 0.5);
    ctx.stroke();

    const lx1 = this.timeToX(this.project.loopStart);
    const lx2 = this.timeToX(this.project.loopEnd);
    ctx.fillStyle = this.project.loopEnabled ? 'rgba(56, 189, 248, 0.35)' : 'rgba(100, 116, 139, 0.25)';
    ctx.fillRect(lx1, 2, Math.max(4, lx2 - lx1), 10);

    ctx.fillStyle = this.project.loopEnabled ? '#38bdf8' : '#94a3b8';
    ctx.fillRect(lx1 - 3, 1, 6, 12);
    ctx.fillRect(lx2 - 3, 1, 6, 12);

    const stepSec = this.pixelsPerSecond > 140 ? 1 : this.pixelsPerSecond > 60 ? 2 : 5;
    const firstSec = Math.floor(this.scrollX / stepSec) * stepSec;
    const endSec = this.xToTime(w);

    ctx.fillStyle = this.lightThemeCanvas ? '#222222' : '#94a3b8';
    ctx.font = '10px Consolas, monospace';
    for (let s = firstSec; s <= endSec; s += stepSec) {
      const x = Math.round(this.timeToX(s));
      if (x < 0) continue;
      ctx.strokeStyle = '#475569';
      ctx.beginPath();
      ctx.moveTo(x, RULER_HEIGHT - 10);
      ctx.lineTo(x, RULER_HEIGHT);
      ctx.stroke();
      ctx.fillText(formatSMPTE(s, this.project.fps, true), x + 4, RULER_HEIGHT - 4);
    }

    for (const reg of this.project.regions) {
      const rx1 = this.timeToX(reg.start);
      const rx2 = this.timeToX(reg.end);
      ctx.fillStyle = 'rgba(34, 197, 94, 0.22)';
      ctx.fillRect(rx1, 13, Math.max(4, rx2 - rx1), 11);
      ctx.strokeStyle = '#22c55e';
      ctx.strokeRect(rx1, 13, Math.max(4, rx2 - rx1), 11);
      ctx.fillStyle = '#86efac';
      ctx.font = 'bold 9px sans-serif';
      ctx.fillText(reg.label, rx1 + 4, 22);
    }

    for (const marker of this.project.markers) {
      const mx = this.timeToX(marker.time);
      if (mx < -20 || mx > w + 20) continue;
      ctx.strokeStyle = marker.color || '#f97316';
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(mx, 14);
      ctx.lineTo(mx, h);
      ctx.stroke();

      const tagW = Math.min(90, marker.label.length * 6 + 14);
      ctx.fillStyle = marker.color || '#f97316';
      ctx.fillRect(mx, 14, tagW, 13);
      ctx.fillStyle = '#090b10';
      ctx.font = 'bold 9px sans-serif';
      ctx.fillText(marker.label, mx + 4, 24);
    }
  }

          drawTrackRow(
    ctx                          ,
    track       ,
    trackIdx        ,
    top        ,
    trackH        ,
    w        
  )       {
    ctx.fillStyle = trackIdx % 2 === 0 ? 'rgba(255,255,255,0.015)' : 'rgba(0,0,0,0.12)';
    ctx.fillRect(0, top, w, trackH);

    // Bottom divider
    ctx.strokeStyle = '#242832';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, top + trackH - 0.5);
    ctx.lineTo(w, top + trackH - 0.5);
    ctx.stroke();

    for (const clip of track.clips) {
      this.drawClipEvent(ctx, clip, track, top, trackH);
    }

    for (let i = 0; i < track.clips.length; i++) {
      for (let j = i + 1; j < track.clips.length; j++) {
        const c1 = track.clips[i];
        const c2 = track.clips[j];
        const c1End = c1.start + c1.duration;
        if (c2.start < c1End - 0.01) {
          const ovStart = c2.start;
          const ovEnd = Math.min(c1End, c2.start + c2.duration);
          this.drawCrossfadeXBox(ctx, ovStart, ovEnd, top, trackH, c1.fadeOutCurve, c2.fadeInCurve);
        }
      }
    }

    if (track.showEnvelope && track.envelopePoints.length > 0) {
      ctx.save();
      ctx.strokeStyle = track.type === 'video' ? '#c084fc' : '#38bdf8';
      ctx.lineWidth = 2;
      ctx.beginPath();
      const sorted = [...track.envelopePoints].sort((a, b) => a.time - b.time);
      sorted.forEach((pt, i) => {
        const px = this.timeToX(pt.time);
        const py = top + 12 + (1 - pt.value) * (trackH - 24);
        if (i === 0) {
          ctx.moveTo(0, py);
          ctx.lineTo(px, py);
        } else {
          ctx.lineTo(px, py);
        }
        if (i === sorted.length - 1) {
          ctx.lineTo(w, py);
        }
      });
      ctx.stroke();

      for (const pt of sorted) {
        const px = this.timeToX(pt.time);
        const py = top + 12 + (1 - pt.value) * (trackH - 24);
        ctx.fillStyle = '#f8fafc';
        ctx.fillRect(px - 4, py - 4, 8, 8);
        ctx.strokeStyle = '#9333ea';
        ctx.strokeRect(px - 4, py - 4, 8, 8);
      }
      ctx.restore();
    }
  }

          drawClipEvent(
    ctx                          ,
    clip           ,
    track       ,
    trackTop        ,
    trackH        
  )       {
    const x = this.timeToX(clip.start);
    const w = Math.max(12, clip.duration * this.pixelsPerSecond);
    const y = trackTop + 3;
    const h = trackH - 6;
    const isSelected = this.selectedClipId === clip.id;

    ctx.save();
    ctx.beginPath();
    ctx.rect(x, y, w, h);
    ctx.clip();

    const baseColor = clip.color || (track.type === 'video' ? '#2563eb' : '#0891b2');
    const bodyGrad = ctx.createLinearGradient(x, y, x, y + h);
    bodyGrad.addColorStop(0, this.hexToRgba(baseColor, clip.muted ? 0.2 : 0.48));
    bodyGrad.addColorStop(1, this.hexToRgba(baseColor, clip.muted ? 0.12 : 0.24));
    ctx.fillStyle = bodyGrad;
    ctx.fillRect(x, y, w, h);

    ctx.fillStyle = this.hexToRgba(baseColor, 0.78);
    ctx.fillRect(x, y, w, 17);

    if (h > 24) {
      if (track.type === 'audio') {
        this.drawAudioWaveformInClip(ctx, clip, x, y + 18, w, h - 19);
      } else {
        this.drawVideoFilmstripInClip(ctx, clip, x, y + 18, w, h - 19);
      }
    }

    const gainNorm = Math.max(0, Math.min(1, clip.gain));
    const gainY = y + 17 + (1 - gainNorm) * Math.max(4, h - 21);
    ctx.strokeStyle = 'rgba(250, 204, 21, 0.75)';
    ctx.lineWidth = 1.2;
    ctx.beginPath();
    ctx.moveTo(x, gainY);
    ctx.lineTo(x + w, gainY);
    ctx.stroke();

    if (clip.fadeIn > 0.01) {
      const fadeW = Math.min(w, clip.fadeIn * this.pixelsPerSecond);
      ctx.beginPath();
      ctx.moveTo(x, y + h);
      const steps = 16;
      for (let s = 0; s <= steps; s++) {
        const ratio = s / steps;
        const fy = y + h - evaluateFadeCurve(clip.fadeInCurve, ratio) * h;
        ctx.lineTo(x + ratio * fadeW, fy);
      }
      ctx.lineTo(x, y);
      ctx.closePath();
      ctx.fillStyle = 'rgba(9, 11, 16, 0.52)';
      ctx.fill();

      ctx.beginPath();
      for (let s = 0; s <= steps; s++) {
        const ratio = s / steps;
        const fy = y + h - evaluateFadeCurve(clip.fadeInCurve, ratio) * h;
        if (s === 0) ctx.moveTo(x, fy);
        else ctx.lineTo(x + ratio * fadeW, fy);
      }
      ctx.strokeStyle = '#38bdf8';
      ctx.lineWidth = 1.6;
      ctx.stroke();

      ctx.fillStyle = '#38bdf8';
      ctx.beginPath();
      ctx.moveTo(x + fadeW - 5, y);
      ctx.lineTo(x + fadeW + 5, y);
      ctx.lineTo(x + fadeW, y + 8);
      ctx.closePath();
      ctx.fill();
    } else {
      ctx.fillStyle = 'rgba(255,255,255,0.45)';
      ctx.fillRect(x + 1, y + 1, 5, 5);
    }

    if (clip.fadeOut > 0.01) {
      const fadeW = Math.min(w, clip.fadeOut * this.pixelsPerSecond);
      const startX = x + w - fadeW;
      const steps = 16;
      ctx.beginPath();
      ctx.moveTo(x + w, y + h);
      for (let s = 0; s <= steps; s++) {
        const ratio = s / steps;
        const fy = y + h - evaluateFadeCurve(clip.fadeOutCurve, 1 - ratio) * h;
        ctx.lineTo(startX + ratio * fadeW, fy);
      }
      ctx.lineTo(x + w, y);
      ctx.closePath();
      ctx.fillStyle = 'rgba(9, 11, 16, 0.52)';
      ctx.fill();

      ctx.beginPath();
      for (let s = 0; s <= steps; s++) {
        const ratio = s / steps;
        const fy = y + h - evaluateFadeCurve(clip.fadeOutCurve, 1 - ratio) * h;
        if (s === 0) ctx.moveTo(startX, fy);
        else ctx.lineTo(startX + ratio * fadeW, fy);
      }
      ctx.strokeStyle = '#38bdf8';
      ctx.lineWidth = 1.6;
      ctx.stroke();

      ctx.fillStyle = '#38bdf8';
      ctx.beginPath();
      ctx.moveTo(startX - 5, y);
      ctx.lineTo(startX + 5, y);
      ctx.lineTo(startX, y + 8);
      ctx.closePath();
      ctx.fill();
    } else {
      ctx.fillStyle = 'rgba(255,255,255,0.45)';
      ctx.fillRect(x + w - 6, y + 1, 5, 5);
    }

    ctx.fillStyle = '#f8fafc';
    ctx.font = 'bold 10.5px "Segoe UI", sans-serif';
    const gainPct = Math.round(clip.gain * 100);
    const label = `${clip.name} (${gainPct}%)`;
    ctx.fillText(label, x + 7, y + 12);

    if (w > 75) {
      if (track.type === 'video') {
        const cropX = x + w - 42;
        ctx.fillStyle = 'rgba(15, 23, 42, 0.78)';
        ctx.fillRect(cropX, y + 2, 17, 13);
        ctx.fillStyle = '#bae6fd';
        ctx.font = 'bold 9px monospace';
        ctx.fillText('[]', cropX + 3, y + 11);
      }
      const fxX = x + w - 22;
      const hasActiveFx = clip.fx && clip.fx.some((f) => f.enabled);
      ctx.fillStyle = hasActiveFx ? '#2563eb' : 'rgba(15, 23, 42, 0.78)';
      ctx.fillRect(fxX, y + 2, 18, 13);
      ctx.fillStyle = hasActiveFx ? '#ffffff' : '#94a3b8';
      ctx.font = 'bold 9px monospace';
      ctx.fillText('fx', fxX + 3, y + 11);
    }

    ctx.restore();

    ctx.save();
    ctx.strokeStyle = isSelected ? '#facc15' : 'rgba(255,255,255,0.25)';
    ctx.lineWidth = isSelected ? 2 : 1;
    ctx.strokeRect(x, y, w, h);
    ctx.restore();
  }

          drawAudioWaveformInClip(
    ctx                          ,
    clip           ,
    x        ,
    y        ,
    w        ,
    h        
  )       {
    const midY = y + h / 2;
    ctx.strokeStyle = 'rgba(255,255,255,0.12)';
    ctx.beginPath();
    ctx.moveTo(x, midY);
    ctx.lineTo(x + w, midY);
    ctx.stroke();

    const peaks = clip.peaks && clip.peaks.length > 0 ? clip.peaks : null;
    const barStep = 3;
    const count = Math.max(1, Math.floor(w / barStep));

    ctx.fillStyle = 'rgba(103, 232, 249, 0.72)';
    for (let i = 0; i < count; i++) {
      const ratio = i / count;
      let amp = 0.4;
      if (peaks) {
        const pIdx = Math.min(peaks.length - 1, Math.floor(ratio * peaks.length));
        amp = peaks[pIdx] * clip.gain;
      } else {
        amp = (0.25 + 0.55 * Math.abs(Math.sin(i * 0.42 + clip.trimIn * 4))) * clip.gain;
      }
      const barH = Math.max(2, Math.min(h - 2, amp * (h - 2)));
      ctx.fillRect(x + i * barStep, midY - barH / 2, 2, barH);
    }
  }

          drawVideoFilmstripInClip(
    ctx                          ,
    clip           ,
    x        ,
    y        ,
    w        ,
    h        
  )       {
    const frameW = 54;
    const count = Math.ceil(w / frameW);
    for (let i = 0; i < count; i++) {
      const fx = x + i * frameW;
      ctx.strokeStyle = 'rgba(255,255,255,0.12)';
      ctx.strokeRect(fx, y + 2, Math.min(frameW - 2, x + w - fx), h - 4);
      ctx.fillStyle = 'rgba(255,255,255,0.18)';
      ctx.fillRect(fx + 4, y + 4, 6, 3);
      ctx.fillRect(fx + 4, y + h - 7, 6, 3);
    }
  }

          drawCrossfadeXBox(
    ctx                          ,
    startSec        ,
    endSec        ,
    trackTop        ,
    trackH        ,
    outCurve               ,
    inCurve               
  )       {
    const x1 = this.timeToX(startSec);
    const x2 = this.timeToX(endSec);
    const w = x2 - x1;
    if (w < 3) return;
    const y = trackTop + 3;
    const h = trackH - 6;

    ctx.save();
    ctx.fillStyle = 'rgba(250, 204, 21, 0.14)';
    ctx.fillRect(x1, y, w, h);
    ctx.strokeStyle = '#facc15';
    ctx.lineWidth = 1.5;

    const steps = 14;
    ctx.beginPath();
    for (let s = 0; s <= steps; s++) {
      const r = s / steps;
      const py = y + h - evaluateFadeCurve(outCurve, 1 - r) * h;
      if (s === 0) ctx.moveTo(x1, py);
      else ctx.lineTo(x1 + r * w, py);
    }
    ctx.stroke();

    ctx.beginPath();
    for (let s = 0; s <= steps; s++) {
      const r = s / steps;
      const py = y + h - evaluateFadeCurve(inCurve, r) * h;
      if (s === 0) ctx.moveTo(x1, py);
      else ctx.lineTo(x1 + r * w, py);
    }
    ctx.stroke();

    if (w > 28) {
      ctx.fillStyle = 'rgba(15, 23, 42, 0.85)';
      ctx.fillRect(x1 + w / 2 - 18, y + h / 2 - 8, 36, 16);
      ctx.fillStyle = '#fde047';
      ctx.font = 'bold 9px monospace';
      ctx.fillText(`${(endSec - startSec).toFixed(1)}s`, x1 + w / 2 - 13, y + h / 2 + 3);
    }
    ctx.restore();
  }

          hexToRgba(hex        , alpha        )         {
    const clean = hex.replace('#', '');
    if (clean.length !== 6) return `rgba(59, 130, 246, ${alpha})`;
    const r = parseInt(clean.slice(0, 2), 16);
    const g = parseInt(clean.slice(2, 4), 16);
    const b = parseInt(clean.slice(4, 6), 16);
    return `rgba(${r}, ${g}, ${b}, ${alpha})`;
  }
}


//# sourceURL=src/ui/timeline.ts

// ─── MODULE: src/ui/dockManager.ts ───
/**
 * OpenNeoveg — Interactive Window Docking, Floating ("Janelas Soltas") & Multi-Monitor OS Window Engine
 * Allows every studio panel (Project Media, Inspector/Pan-Crop/Settings, Video Preview,
 * Master Mixer/Scopes, and Timeline) to be:
 * - Resized via interactive horizontal & vertical splitter bars while docked
 * - Detached ("Solta" / Undocked) inside the workspace by dragging its `⋮⋮` header grip or clicking Float
 * - Popped out into a TRUE Native Windows OS Window (`NeovegPopoutWindow`) that can be dragged freely
 *   to any secondary/tertiary monitor (`Screen.AllScreens`) via `[data-win-action="popout"]` or by dragging
 *   a floating panel past the outer edge of the main window!
 * - Synchronized in real time across all monitors via `BroadcastChannel('neoveg_multimonitor_sync')`
 */
class NeovegHostDockManager {
          panels                          = new Map();
          topZ = 200;
          onLayoutResize            ;
          popoutWindows                             = new Map();
          syncChannel                          = null;
                  activePopoutPanelId               ;

          activeDragPanelId                = null;
          dragType                                                                                          = null;
          dragStartX = 0;
          dragStartY = 0;
          initialRect               = { x: 0, y: 0, w: 420, h: 320 };

  constructor(onLayoutResize            ) {
    this.onLayoutResize = onLayoutResize;
    const params = new URLSearchParams(window.location.search);
    this.activePopoutPanelId = params.get('popout');
    this.initDefaultPanelStates();

    try {
      this.syncChannel = new BroadcastChannel('neoveg_multimonitor_sync');
      this.syncChannel.addEventListener('message', (ev) => {
        const msg = ev.data;
        if (!msg || typeof msg !== 'object') return;
        if (msg.type === 'REDOCK_PANEL' && !this.activePopoutPanelId) {
          this.restoreFromOSPopout(msg.panelId);
        }
      });
    } catch {
      // Fallback if BroadcastChannel is unavailable
    }
  }

          initDefaultPanelStates()       {
    const defaults               = [
      {
        id: 'pane-media',
        title: 'Project Media / FX / Generators',
        floating: false,
        poppedOutToOS: false,
        maximized: false,
        minimized: false,
        rect: { x: 40, y: 70, w: 430, h: 360 },
        dockOrder: 1,
        dockFlex: 28,
      },
      {
        id: 'pane-inspector',
        title: 'Event Pan/Crop, FX, Resolve & Configurações',
        floating: false,
        poppedOutToOS: false,
        maximized: false,
        minimized: false,
        rect: { x: 490, y: 70, w: 420, h: 360 },
        dockOrder: 2,
        dockFlex: 28,
      },
      {
        id: 'pane-preview',
        title: 'Video Preview Monitor',
        floating: false,
        poppedOutToOS: false,
        maximized: false,
        minimized: false,
        rect: { x: 930, y: 70, w: 480, h: 370 },
        dockOrder: 3,
        dockFlex: 32,
      },
      {
        id: 'pane-mixer',
        title: 'Master Audio Bus & RGB Scopes',
        floating: false,
        poppedOutToOS: false,
        maximized: false,
        minimized: false,
        rect: { x: 1180, y: 450, w: 210, h: 360 },
        dockOrder: 4,
        dockFlex: 12,
      },
      {
        id: 'pane-timeline',
        title: 'Multi-Track Timeline & Envelopes',
        floating: false,
        poppedOutToOS: false,
        maximized: false,
        minimized: false,
        rect: { x: 60, y: 430, w: 1280, h: 380 },
        dockOrder: 5,
        dockFlex: 100,
      },
    ];

    for (const p of defaults) {
      this.panels.set(p.id, p);
    }
  }

         initDOM()       {
    // If this window was spawned as a dedicated OS Multi-Monitor Popout Window (`?popout=pane-...`)
    if (this.activePopoutPanelId) {
      this.configureAsDedicatedOSPopoutWindow(this.activePopoutPanelId);
      return;
    }

    for (const [id, state] of this.panels.entries()) {
      const el = document.getElementById(id);
      if (!el) continue;

      const hdr = el.querySelector('.neoveg-win-titlebar')                      ;
      const winControls = el.querySelector('.win-controls')                      ;
      const btnFloat = el.querySelector('[data-win-action="float"]')                            ;
      const btnMax = el.querySelector('[data-win-action="maximize"]')                            ;
      const btnMin = el.querySelector('[data-win-action="minimize"]')                            ;

      // Inject Multi-Monitor Native OS Popout Button if not already in DOM
      let btnPopout = el.querySelector('[data-win-action="popout"]')                            ;
      if (!btnPopout && winControls) {
        btnPopout = document.createElement('button');
        btnPopout.className = 'win-ctrl-btn popout-monitor-btn';
        btnPopout.setAttribute('data-win-action', 'popout');
        btnPopout.title = 'Destacar como Janela Nativa do Windows para arrastar para OUTRO MONITOR';
        btnPopout.innerHTML = `${Neoveg_ICONS.popoutMonitor}`;
        winControls.insertBefore(btnPopout, btnFloat);
      }

      btnPopout?.addEventListener('click', (e) => {
        e.stopPropagation();
        this.popoutPanelToMonitor(id, e.screenX, e.screenY);
      });

      el.addEventListener('mousedown', () => {
        if (state.floating) {
          this.bringToFront(id);
        }
      });

      btnFloat?.addEventListener('click', (e) => {
        e.stopPropagation();
        this.toggleFloat(id);
      });

      btnMax?.addEventListener('click', (e) => {
        e.stopPropagation();
        this.toggleMaximize(id);
      });

      btnMin?.addEventListener('click', (e) => {
        e.stopPropagation();
        this.toggleMinimize(id);
      });

      hdr?.addEventListener('dblclick', (e) => {
        if ((e.target               ).tagName === 'BUTTON') return;
        this.toggleFloat(id);
      });

      hdr?.addEventListener('mousedown', (e) => {
        if (e.button !== 0) return;
        const targetTag = (e.target               ).tagName;
        if (targetTag === 'BUTTON' || targetTag === 'SELECT' || targetTag === 'INPUT' || targetTag === 'SVG' || targetTag === 'PATH') return;

        e.preventDefault();
        this.activeDragPanelId = id;
        this.dragStartX = e.clientX;
        this.dragStartY = e.clientY;

        if (state.floating) {
          this.bringToFront(id);
          this.dragType = 'move';
          this.initialRect = { ...state.rect };
        } else {
          this.dragType = 'undock-threshold';
          const domRect = el.getBoundingClientRect();
          this.initialRect = {
            x: Math.max(16, domRect.left),
            y: Math.max(52, domRect.top),
            w: Math.max(260, Math.min(960, domRect.width)),
            h: Math.max(220, Math.min(620, domRect.height)),
          };
        }
      });

      const handles                                                     = [
        { cls: 'win-resize-se', mode: 'resize-se' },
        { cls: 'win-resize-e', mode: 'resize-e' },
        { cls: 'win-resize-s', mode: 'resize-s' },
        { cls: 'win-resize-w', mode: 'resize-w' },
      ];
      for (const h of handles) {
        const handleEl = document.createElement('div');
        handleEl.className = `win-resize-handle ${h.cls}`;
        handleEl.addEventListener('mousedown', (e) => {
          if (!state.floating || e.button !== 0) return;
          e.preventDefault();
          e.stopPropagation();
          this.bringToFront(id);
          this.activeDragPanelId = id;
          this.dragType = h.mode;
          this.dragStartX = e.clientX;
          this.dragStartY = e.clientY;
          this.initialRect = { ...state.rect };
        });
        el.appendChild(handleEl);
      }
    }

    window.addEventListener('mousemove', (e) => this.onGlobalMouseMove(e));
    window.addEventListener('mouseup', (e) => this.onGlobalMouseUp(e));

    this.initWorkspaceSplitters();
    this.applyAllPanelStyles();
  }

  /**
   * Configures this browser/WebView2 instance when running inside a native OS Multi-Monitor Popout Window.
   */
          configureAsDedicatedOSPopoutWindow(panelId        )       {
    document.body.classList.add('is-multimonitor-popout');
    const state = this.panels.get(panelId);
    if (state) {
      document.title = `OpenNeoveg 21 — ${state.title} [Janela Multi-Monitor]`;
    }

    // Hide topbar, statusbar, welcome modal, and all other panels
    document.querySelector('.neoveg-topbar')?.classList.add('hidden');
    document.querySelector('.neoveg-statusbar')?.classList.add('hidden');
    document.getElementById('modal-welcome')?.classList.add('hidden');
    document.getElementById('workspace-h-splitter')?.classList.add('hidden');
    document.querySelectorAll('.dock-v-splitter').forEach((s) => s.classList.add('hidden'));

    for (const id of this.panels.keys()) {
      const el = document.getElementById(id);
      if (!el) continue;
      if (id === panelId) {
        el.style.position = 'fixed';
        el.style.inset = '0';
        el.style.width = '100vw';
        el.style.height = '100vh';
        el.style.zIndex = '999';
        const btnFloat = el.querySelector('[data-win-action="float"]')                      ;
        if (btnFloat) {
          btnFloat.innerHTML = `${Neoveg_ICONS.dockWindow}<span>Acoplar na Janela Principal</span>`;
          btnFloat.onclick = () => {
            this.syncChannel?.postMessage({ type: 'REDOCK_PANEL', panelId });
            window.close();
          };
        }
      } else {
        el.classList.add('hidden');
      }
    }

    window.addEventListener('beforeunload', () => {
      this.syncChannel?.postMessage({ type: 'REDOCK_PANEL', panelId });
    });
  }

  /**
   * Detaches a studio panel into a real native OS Window (`Form` / `window.open`) that can be dragged
   * freely across multiple monitors (`Screen.AllScreens`).
   */
         popoutPanelToMonitor(panelId        , screenX         , screenY         )       {
    const state = this.panels.get(panelId);
    if (!state) return;

    // If already popped out, focus existing OS window
    const existingWin = this.popoutWindows.get(panelId);
    if (existingWin && !existingWin.closed) {
      existingWin.focus();
      return;
    }

    const theme = document.documentElement.getAttribute('data-theme') || 'qt-neoveg21';
    const w = Math.max(420, state.rect.w || 580);
    const h = Math.max(320, state.rect.h || 420);
    const left = screenX !== undefined ? Math.max(0, screenX - 120) : window.screenX + 80;
    const top = screenY !== undefined ? Math.max(0, screenY - 20) : window.screenY + 80;

    // Notify C# native host (if running in WebView2) about desired multi-monitor window coordinates
    const wv = (window                                                                                 ).chrome?.webview;
    wv?.postMessage({
      action: 'prepare_popout_bounds',
      panelId,
      title: state.title,
      x: Math.round(left),
      y: Math.round(top),
      width: Math.round(w),
      height: Math.round(h),
    });

    const popUrl = `${window.location.origin}/?popout=${encodeURIComponent(panelId)}&theme=${encodeURIComponent(theme)}`;
    const features = `popup=yes,width=${Math.round(w)},height=${Math.round(h)},left=${Math.round(left)},top=${Math.round(top)},resizable=yes`;
    const newWin = window.open(popUrl, `neoveg_${panelId}`, features);

    if (newWin) {
      this.popoutWindows.set(panelId, newWin);
      state.poppedOutToOS = true;
      state.floating = false;
      this.applyAllPanelStyles();
      this.onLayoutResize();

      const watchTimer = window.setInterval(() => {
        if (newWin.closed) {
          window.clearInterval(watchTimer);
          this.restoreFromOSPopout(panelId);
        }
      }, 600);
    }
  }

         restoreFromOSPopout(panelId        )       {
    const state = this.panels.get(panelId);
    if (!state) return;
    const win = this.popoutWindows.get(panelId);
    if (win && !win.closed) {
      try {
        win.close();
      } catch {
        // Ignore
      }
    }
    this.popoutWindows.delete(panelId);
    state.poppedOutToOS = false;
    state.floating = false;
    this.applyAllPanelStyles();
    this.onLayoutResize();
  }

          onGlobalMouseMove(e            )       {
    if (!this.activeDragPanelId || !this.dragType) return;
    const state = this.panels.get(this.activeDragPanelId);
    if (!state) return;

    const dx = e.clientX - this.dragStartX;
    const dy = e.clientY - this.dragStartY;

    if (this.dragType === 'undock-threshold') {
      if (Math.hypot(dx, dy) > 16) {
        state.floating = true;
        state.maximized = false;
        state.rect = {
          x: Math.max(10, e.clientX - Math.min(180, this.initialRect.w / 2)),
          y: Math.max(48, e.clientY - 14),
          w: this.initialRect.w,
          h: this.initialRect.h,
        };
        this.initialRect = { ...state.rect };
        this.dragStartX = e.clientX;
        this.dragStartY = e.clientY;
        this.dragType = 'move';
        this.bringToFront(state.id);
        this.applyAllPanelStyles();
        this.onLayoutResize();
      }
      return;
    }

    if (this.dragType === 'move') {
      state.rect.x = this.initialRect.x + dx;
      state.rect.y = Math.max(36, this.initialRect.y + dy);
      this.applyPanelStyle(state);
    } else if (this.dragType === 'resize-se') {
      state.rect.w = Math.max(200, this.initialRect.w + dx);
      state.rect.h = Math.max(150, this.initialRect.h + dy);
      this.applyPanelStyle(state);
      this.onLayoutResize();
    } else if (this.dragType === 'resize-e') {
      state.rect.w = Math.max(200, this.initialRect.w + dx);
      this.applyPanelStyle(state);
      this.onLayoutResize();
    } else if (this.dragType === 'resize-s') {
      state.rect.h = Math.max(150, this.initialRect.h + dy);
      this.applyPanelStyle(state);
      this.onLayoutResize();
    } else if (this.dragType === 'resize-w') {
      const newW = Math.max(200, this.initialRect.w - dx);
      state.rect.x = this.initialRect.x + (this.initialRect.w - newW);
      state.rect.w = newW;
      this.applyPanelStyle(state);
      this.onLayoutResize();
    }
  }

          onGlobalMouseUp(e            )       {
    if (this.activeDragPanelId) {
      const panelId = this.activeDragPanelId;
      const mode = this.dragType;
      const state = this.panels.get(panelId);
      this.activeDragPanelId = null;
      this.dragType = null;

      // If user dragged a floating window past the left/right/top/bottom edge of the main window,
      // automatically pop it out into a Native OS Multi-Monitor Window so it can go to Monitor 2!
      if (
        mode === 'move' &&
        state &&
        state.floating &&
        (e.clientX <= 2 || e.clientX >= window.innerWidth - 2 || e.clientY <= 2 || e.clientY >= window.innerHeight - 2)
      ) {
        this.popoutPanelToMonitor(panelId, e.screenX, e.screenY);
        return;
      }

      if (state && state.floating) {
        state.rect.x = Math.max(4, Math.min(window.innerWidth - 120, state.rect.x));
        state.rect.y = Math.max(42, Math.min(window.innerHeight - 60, state.rect.y));
        this.applyPanelStyle(state);
      }
      this.onLayoutResize();
    }
  }

         toggleFloat(panelId        , forceState          )       {
    const state = this.panels.get(panelId);
    if (!state) return;
    const el = document.getElementById(panelId);

    const nextFloating = forceState !== undefined ? forceState : !state.floating;
    if (nextFloating && !state.floating && el) {
      const domRect = el.getBoundingClientRect();
      state.rect = {
        x: Math.max(24, Math.min(window.innerWidth - 360, domRect.left + 18)),
        y: Math.max(54, Math.min(window.innerHeight - 260, domRect.top + 18)),
        w: Math.max(240, Math.min(920, domRect.width)),
        h: Math.max(220, Math.min(580, domRect.height)),
      };
    }

    state.floating = nextFloating;
    state.maximized = false;
    state.minimized = false;
    if (state.floating) {
      this.bringToFront(panelId);
    }
    this.applyAllPanelStyles();
    this.onLayoutResize();
  }

         toggleMaximize(panelId        )       {
    const state = this.panels.get(panelId);
    if (!state) return;
    state.maximized = !state.maximized;
    state.minimized = false;
    if (state.maximized) {
      this.bringToFront(panelId);
    }
    this.applyAllPanelStyles();
    this.onLayoutResize();
  }

         toggleMinimize(panelId        )       {
    const state = this.panels.get(panelId);
    if (!state) return;
    state.minimized = !state.minimized;
    this.applyAllPanelStyles();
    this.onLayoutResize();
  }

         floatAllPanels()       {
    const presets                               = {
      'pane-media': { x: 24, y: 58, w: 390, h: 340 },
      'pane-inspector': { x: 430, y: 58, w: 400, h: 340 },
      'pane-preview': { x: 845, y: 58, w: 460, h: 340 },
      'pane-mixer': { x: 1315, y: 58, w: 150, h: 340 },
    };
    for (const [id, rect] of Object.entries(presets)) {
      const st = this.panels.get(id);
      if (st) {
        st.floating = true;
        st.maximized = false;
        st.minimized = false;
        st.rect = { ...rect };
      }
    }
    this.applyAllPanelStyles();
    this.onLayoutResize();
  }

         resetDefaultLayout()       {
    for (const id of Array.from(this.popoutWindows.keys())) {
      this.restoreFromOSPopout(id);
    }
    this.initDefaultPanelStates();
    const upper = document.getElementById('neoveg-upper-workspace');
    if (upper) upper.style.height = '48%';
    const trackCol = document.getElementById('timeline-left-column');
    if (trackCol) trackCol.style.width = '255px';
    this.applyAllPanelStyles();
    this.onLayoutResize();
  }

          bringToFront(panelId        )       {
    this.topZ++;
    const el = document.getElementById(panelId);
    if (el) {
      el.style.zIndex = String(this.topZ);
    }
    document.querySelectorAll('.neoveg-dock-window').forEach((w) => w.classList.remove('win-focused'));
    el?.classList.add('win-focused');
  }

          applyAllPanelStyles()       {
    for (const state of this.panels.values()) {
      this.applyPanelStyle(state);
    }
    const upper = document.getElementById('neoveg-upper-workspace');
    const hSplitter = document.getElementById('workspace-h-splitter');
    const anyTopDocked = ['pane-media', 'pane-inspector', 'pane-preview', 'pane-mixer'].some((id) => {
      const st = this.panels.get(id);
      return st && !st.floating && !st.poppedOutToOS;
    });
    if (upper) {
      upper.classList.toggle('all-undocked', !anyTopDocked);
    }
    if (hSplitter) {
      hSplitter.classList.toggle('hidden', !anyTopDocked);
    }
  }

          applyPanelStyle(state            )       {
    const el = document.getElementById(state.id);
    if (!el) return;

    if (state.poppedOutToOS) {
      el.classList.add('hidden');
      return;
    } else {
      el.classList.remove('hidden');
    }

    const btnFloat = el.querySelector('[data-win-action="float"]')                      ;
    const btnMax = el.querySelector('[data-win-action="maximize"]')                      ;
    const btnMin = el.querySelector('[data-win-action="minimize"]')                      ;

    el.classList.toggle('is-floating', state.floating);
    el.classList.toggle('is-maximized', state.maximized);
    el.classList.toggle('is-minimized', state.minimized);

    if (btnMin) {
      btnMin.innerHTML = Neoveg_ICONS.minimize;
    }
    if (btnFloat) {
      btnFloat.innerHTML = state.floating ? `${Neoveg_ICONS.dockWindow}` : `${Neoveg_ICONS.floatWindow}`;
      btnFloat.title = state.floating
        ? 'Acoplar janela de volta na grade (Dock)'
        : 'Soltar janela como flutuante (ou arraste até a borda da tela para jogar em outro monitor)';
    }
    if (btnMax) {
      btnMax.innerHTML = state.maximized ? Neoveg_ICONS.restore : Neoveg_ICONS.maximize;
    }

    if (state.maximized) {
      el.style.position = 'fixed';
      el.style.left = '8px';
      el.style.top = '48px';
      el.style.width = `${window.innerWidth - 16}px`;
      el.style.height = `${window.innerHeight - 76}px`;
      el.style.flex = 'none';
      return;
    }

    if (state.floating) {
      el.style.position = 'fixed';
      el.style.left = `${state.rect.x}px`;
      el.style.top = `${state.rect.y}px`;
      el.style.width = `${state.rect.w}px`;
      el.style.height = state.minimized ? '30px' : `${state.rect.h}px`;
      el.style.flex = 'none';
    } else {
      el.style.position = 'relative';
      el.style.left = '';
      el.style.top = '';
      el.style.width = '';
      el.style.height = '';
      el.style.zIndex = '';
      if (state.id !== 'pane-timeline') {
        el.style.flex = `${state.dockFlex} 1 0%`;
      } else {
        el.style.flex = '1 1 auto';
      }
    }
  }

          initWorkspaceSplitters()       {
    const hSplitter = document.getElementById('workspace-h-splitter');
    const upper = document.getElementById('neoveg-upper-workspace');
    if (hSplitter && upper) {
      let draggingH = false;
      hSplitter.addEventListener('mousedown', (e) => {
        if (e.button !== 0) return;
        e.preventDefault();
        draggingH = true;
        document.body.style.cursor = 'ns-resize';
      });
      window.addEventListener('mousemove', (e) => {
        if (!draggingH) return;
        const minH = 170;
        const maxH = window.innerHeight - 180;
        const newH = Math.max(minH, Math.min(maxH, e.clientY - 42));
        upper.style.height = `${newH}px`;
        this.onLayoutResize();
      });
      window.addEventListener('mouseup', () => {
        if (draggingH) {
          draggingH = false;
          document.body.style.cursor = '';
          this.onLayoutResize();
        }
      });
    }

    document.querySelectorAll('.dock-v-splitter').forEach((splitter) => {
      let draggingV = false;
      let leftId = '';
      let rightId = '';
      let startX = 0;
      let startLeftFlex = 25;
      let startRightFlex = 25;

      splitter.addEventListener('mousedown', (e) => {
        const me = e              ;
        if (me.button !== 0) return;
        me.preventDefault();
        leftId = (splitter               ).dataset.leftPane || '';
        rightId = (splitter               ).dataset.rightPane || '';
        const pLeft = this.panels.get(leftId);
        const pRight = this.panels.get(rightId);
        if (!pLeft || !pRight || pLeft.floating || pRight.floating) return;

        draggingV = true;
        startX = me.clientX;
        startLeftFlex = pLeft.dockFlex;
        startRightFlex = pRight.dockFlex;
        document.body.style.cursor = 'ew-resize';
      });

      window.addEventListener('mousemove', (e) => {
        if (!draggingV) return;
        const pLeft = this.panels.get(leftId);
        const pRight = this.panels.get(rightId);
        if (!pLeft || !pRight) return;

        const totalWidth = upper?.clientWidth || window.innerWidth;
        const deltaPct = ((e.clientX - startX) / Math.max(400, totalWidth)) * 100;
        const sumFlex = startLeftFlex + startRightFlex;
        const newLeft = Math.max(10, Math.min(sumFlex - 10, startLeftFlex + deltaPct));
        const newRight = sumFlex - newLeft;

        pLeft.dockFlex = newLeft;
        pRight.dockFlex = newRight;
        this.applyPanelStyle(pLeft);
        this.applyPanelStyle(pRight);
        this.onLayoutResize();
      });

      window.addEventListener('mouseup', () => {
        if (draggingV) {
          draggingV = false;
          document.body.style.cursor = '';
          this.onLayoutResize();
        }
      });
    });

    const trackSplitter = document.getElementById('timeline-track-splitter');
    const trackCol = document.getElementById('timeline-left-column');
    if (trackSplitter && trackCol) {
      let draggingTr = false;
      trackSplitter.addEventListener('mousedown', (e) => {
        if (e.button !== 0) return;
        e.preventDefault();
        draggingTr = true;
        document.body.style.cursor = 'ew-resize';
      });
      window.addEventListener('mousemove', (e) => {
        if (!draggingTr) return;
        const rect = trackCol.parentElement?.getBoundingClientRect();
        const offsetLeft = rect ? rect.left : 0;
        const newW = Math.max(180, Math.min(460, e.clientX - offsetLeft));
        trackCol.style.width = `${newW}px`;
        this.onLayoutResize();
      });
      window.addEventListener('mouseup', () => {
        if (draggingTr) {
          draggingTr = false;
          document.body.style.cursor = '';
          this.onLayoutResize();
        }
      });
    }
  }
}


//# sourceURL=src/ui/dockManager.ts

// ─── MODULE: src/main.ts ───
/**
 * OpenNeoveg 21 — Master Studio Application Controller (TypeScript / Qt Design System)
 * Integrates:
 * - Welcome Project Hub (Starts with Empty Project + Keeps Demo Project + Advanced Neovegject Properties)
 * - Full Interactive Neoveg Event Pan/Crop Workspace with Keyframe Timeline & Draggable 'F' Crop Frame
 * - Video FX, Frei0r & CorridorKey Catalog with Live Human Face Portrait Demo Thumbnails
 * - LADSPA Audio DSP Plugins + Fairlight LUFS / Premiere Essential Sound Auto-Ducking
 * - DaVinci Resolve 3-Way Color Wheels, Node Graph & Premiere Pro Edit Tools (Razor, Slip, Slide, Rate Stretch)
 * - Classic Classic NLE Themes (Neoveg 4.0 XP Gray, Neoveg 7.0 Silver, Neoveg 13 Charcoal, Neoveg 18, Neoveg 21 Qt, Resolve, Premiere)
 * - Neoveg Frequent Problems Diagnostic & Auto-Fixer + `log_erros.txt` Live Inspector
 */







class NeovegStudioApp {
         project              ;
         mediaBin               = [];
         savedProjects                     = [];
         scriptPresets                                                                      = {};
         dockManager                   ;
         activeDragPayload                             = null;

          compositor                  ;
          audioEngine                   ;
          timeline                          ;
          multiMonitorBus                          = null;

          isPlaying = false;
          shuttleSpeed = 1.0;
          lastFrameTs = 0;
          activeLeftTab                                                            = 'fx';
          activeCenterTab                                                             = 'pancrop';
          showPanCropGizmo = true;

  constructor() {
    // Starts with an EMPTY project by default (Demo project is preserved in projects/ & Welcome Hub)
    this.project = this.createEmptyProjectState();
  }

         createEmptyProjectState(custom                        )               {
    return {
      name: custom?.name || 'Novo_Projeto_Vazio.veg',
      isDemo: false,
      width: custom?.width || 1920,
      height: custom?.height || 1080,
      fps: custom?.fps || 29.97,
      pixelAspect: custom?.pixelAspect || 1.0,
      fieldOrder: custom?.fieldOrder || 'none',
      pixelFormat: custom?.pixelFormat || '8bit-rec709',
      colorSpace: custom?.colorSpace || 'Rec.709',
      motionBlurType: custom?.motionBlurType || 'gaussian',
      deinterlaceMethod: custom?.deinterlaceMethod || 'blend',
      resampleMode: custom?.resampleMode || 'disable',
      sampleRate: custom?.sampleRate || 48000,
      audioBitDepth: custom?.audioBitDepth || 24,
      masterBusMode: custom?.masterBusMode || 'stereo',
      bpm: custom?.bpm || 120,
      playhead: 0.0,
      loopStart: 0.0,
      loopEnd: 8.0,
      loopEnabled: false,
      snapEnabled: true,
      rippleEnabled: false,
      autoCrossfadeEnabled: true,
      previewQuality: 'best',
      splitScreenBypass: false,
      showSafeAreas: false,
      showGrid: false,
      masterVolume: 1.0,
      markers: [],
      regions: [],
      tracks: [
        {
          id: 'track_v1',
          name: 'Video Overlay / Titles',
          type: 'video',
          color: '#3daee9',
          level: 1.0,
          pan: 0,
          blendMode: 'sourceAlpha',
          muted: false,
          solo: false,
          locked: false,
          armed: false,
          showEnvelope: false,
          envelopePoints: [
            { time: 0, value: 1.0 },
            { time: 8, value: 1.0 },
          ],
          fx: [],
          clips: [],
        },
        {
          id: 'track_v2',
          name: 'Video Main Program',
          type: 'video',
          color: '#2563eb',
          level: 1.0,
          pan: 0,
          blendMode: 'sourceAlpha',
          muted: false,
          solo: false,
          locked: false,
          armed: false,
          showEnvelope: false,
          envelopePoints: [
            { time: 0, value: 1.0 },
            { time: 8, value: 1.0 },
          ],
          fx: [],
          clips: [],
        },
        {
          id: 'track_a1',
          name: 'Audio Stereo Master',
          type: 'audio',
          color: '#0891b2',
          level: 1.0,
          pan: 0,
          blendMode: 'sourceAlpha',
          muted: false,
          solo: false,
          locked: false,
          armed: false,
          showEnvelope: false,
          envelopePoints: [
            { time: 0, value: 1.0 },
            { time: 8, value: 1.0 },
          ],
          fx: [],
          clips: [],
        },
      ],
    };
  }

         async init()                {
    const previewCanvas = document.getElementById('neoveg-preview-canvas')                     ;
    const scopeCanvas = document.getElementById('neoveg-scope-canvas')                     ;
    const timelineCanvas = document.getElementById('neoveg-timeline-canvas')                     ;

    this.compositor = new NeovegCompositor(previewCanvas, scopeCanvas);
    this.audioEngine = new NeovegAudioEngine();
    this.timeline = new NeovegTimelineController(timelineCanvas, this.project, {
      onPlayheadChange: (newTime) => {
        this.project.playhead = Math.max(0, newTime);
        this.updateTimecodeReadouts();
        this.compositor.syncVideos(this.project, this.isPlaying);
        this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
        this.updateMasterMeters();
        this.broadcastMultiMonitorSync('PLAYHEAD');
      },
      onSelectClip: () => {
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
      },
      onProjectMutated: (reason) => {
        this.setStatusMessage(reason);
        this.renderTrackHeaders();
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
        this.broadcastMultiMonitorSync('PROJECT');
      },
      onOpenClipFX: (clip) => {
        this.timeline.selectedClipId = clip.id;
        this.activeCenterTab = 'fxchain';
        this.syncCenterTabButtons();
        this.renderInspectorPanel();
      },
      onOpenClipPanCrop: (clip) => {
        this.timeline.selectedClipId = clip.id;
        this.activeCenterTab = 'pancrop';
        this.showPanCropGizmo = true;
        this.syncCenterTabButtons();
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, clip, true);
      },
      onContextMenuClip: (clip, track, x, y) => {
        this.showClipContextMenu(clip, track, x, y);
      },
      onRazorSplitAtTime: (clip, _track, splitTime) => {
        this.project.playhead = splitTime;
        this.timeline.selectedClipId = clip.id;
        this.splitAtPlayhead();
      },
      onTrackHeightChanged: () => {
        this.renderTrackHeaders();
      },
      onTimelineDrop: (dropTime, targetTrack, event) => {
        this.handleTimelineDrop(dropTime, targetTrack, event);
      },
      getActiveDragPayload: () => this.activeDragPayload,
    });

    this.dockManager = new NeovegDockManager(() => {
      this.timeline.render();
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
    });
    this.dockManager.initDOM();

    this.initMultiMonitorSyncBus();
    this.bindToolbarAndShortcuts();
    this.renderLeftDock();
    this.renderTrackHeaders();
    this.renderInspectorPanel();
    this.timeline.recalculateAutoCrossfades();
    this.timeline.render();
    this.updateTimecodeReadouts();
    this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);

    await this.fetchBackendTelemetryAndAssets();

    // Apply initial theme or tab if passed in URL (for Multi-Monitor Popout Windows)
    const params = new URLSearchParams(window.location.search);
    const urlTheme = params.get('theme');
    if (urlTheme) {
      this.applyTheme(urlTheme);
    }
    const urlTab = params.get('tab');
    if (urlTab === 'settings') {
      this.activeCenterTab = 'settings';
      this.syncCenterTabButtons();
      this.renderInspectorPanel();
    }

    // Only open Welcome Modal on the Main Studio Window (not on Secondary Monitor Popout Windows)
    if (!this.dockManager.activePopoutPanelId) {
      this.openWelcomeModal();
    } else {
      this.multiMonitorBus?.postMessage({ type: 'REQUEST_FULL_STATE' });
    }

    requestAnimationFrame((ts) => this.studioLoop(ts));
  }

          initMultiMonitorSyncBus()       {
    try {
      this.multiMonitorBus = new BroadcastChannel('neoveg_studio_sync');
      this.multiMonitorBus.addEventListener('message', (ev) => {
        const msg = ev.data;
        if (!msg || typeof msg !== 'object') return;
        if (msg.type === 'REQUEST_FULL_STATE' && !this.dockManager.activePopoutPanelId) {
          this.broadcastMultiMonitorSync('PROJECT');
        } else if (msg.type === 'SYNC_PROJECT' && msg.project) {
          this.project = msg.project;
          this.timeline.setProject(this.project);
          this.renderTrackHeaders();
          this.renderInspectorPanel();
          this.updateTimecodeReadouts();
          this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
        } else if (msg.type === 'SYNC_PLAYHEAD' && typeof msg.playhead === 'number') {
          this.project.playhead = msg.playhead;
          this.updateTimecodeReadouts();
          this.timeline.render();
          this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
        } else if (msg.type === 'SYNC_DRAG_PAYLOAD') {
          this.activeDragPayload = msg.payload || null;
        }
      });
    } catch {
      // BroadcastChannel optional fallback
    }
  }

          broadcastMultiMonitorSync(kind                        )       {
    if (!this.multiMonitorBus) return;
    try {
      if (kind === 'PLAYHEAD') {
        this.multiMonitorBus.postMessage({ type: 'SYNC_PLAYHEAD', playhead: this.project.playhead });
      } else {
        this.multiMonitorBus.postMessage({ type: 'SYNC_PROJECT', project: this.project });
      }
    } catch {
      // Ignore
    }
  }

         openSettingsTab()       {
    document.getElementById('modal-welcome')?.classList.add('hidden');
    this.activeCenterTab = 'settings';
    this.syncCenterTabButtons();
    this.renderInspectorPanel();
    this.setStatusMessage('Aba de Configurações Avançadas do Projeto e Preferências aberta.');
  }

          syncCenterTabButtons()       {
    document.querySelectorAll('[data-center-tab]').forEach((b) => {
      b.classList.toggle('active', (b               ).dataset.centerTab === this.activeCenterTab);
    });
  }

          async fetchBackendTelemetryAndAssets()                {
    try {
      const [statusRes, mediaRes, scriptsRes, projRes] = await Promise.all([
        fetch('/api/status').then((r) => r.json()),
        fetch('/api/media/list').then((r) => r.json()),
        fetch('/api/scripts/presets').then((r) => r.json()),
        fetch('/api/projects/list').then((r) => r.json()),
      ]);

      if (statusRes.ok) {
        const badge = document.getElementById('engine-status-pill');
        if (badge) {
          badge.textContent = `Rust Core (${statusRes.rust_core?.version || '1.0'}) • Python ${statusRes.python_version} • FFmpeg/Frei0r/LADSPA • CorridorKey Ready`;
        }
      }

      if (mediaRes.ok && Array.isArray(mediaRes.media)) {
        this.mediaBin = mediaRes.media.map((m            ) => ({
          ...m,
          color: m.type === 'audio' ? '#0891b2' : '#2563eb',
        }));
        this.renderLeftDock();
      }

      if (scriptsRes.ok && scriptsRes.presets) {
        this.scriptPresets = scriptsRes.presets;
      }

      if (projRes.ok && Array.isArray(projRes.projects)) {
        this.savedProjects = projRes.projects;
        this.renderWelcomeProjectsList();
      }
    } catch (err) {
      this.logErrorToBackend('STARTUP_FETCH', String(err));
    }
  }

          async logErrorToBackend(category        , message        , detail = '')                {
    try {
      await fetch('/api/diagnostics/log-error', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ category, message, detail }),
      });
    } catch {
      // Ignore network log fallback
    }
  }

         async loadSavedProjectByFile(fileName        )                {
    try {
      const res = await fetch('/api/projects/load', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ fileName }),
      }).then((r) => r.json());

      if (res.ok && res.project) {
        this.project = res.project;
        // Attach real PCM peaks to matching audio clips
        for (const asset of this.mediaBin) {
          if (asset.peaks && asset.peaks.length > 0) {
            for (const tr of this.project.tracks) {
              for (const c of tr.clips) {
                if (c.mediaFile === asset.fileName) {
                  c.peaks = asset.peaks;
                }
              }
            }
          }
        }
        const firstVideoClip = this.project.tracks
          .find((t) => t.type === 'video' && t.clips.length > 0)
          ?.clips[0];
        this.timeline.selectedClipId = firstVideoClip?.id || null;
        this.timeline.selectedTrackId = this.project.tracks[0]?.id || null;

        this.timeline.setProject(this.project);
        this.renderTrackHeaders();
        this.renderInspectorPanel();
        this.updateTimecodeReadouts();
        this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
        document.getElementById('modal-welcome')?.classList.add('hidden');
        this.setStatusMessage(`Projeto carregado: ${this.project.name} (${this.project.width}x${this.project.height} @ ${this.project.fps}fps)`);
      }
    } catch (err) {
      this.logErrorToBackend('PROJECT_LOAD', String(err));
    }
  }

         createEmptyProjectFlow()       {
    this.project = this.createEmptyProjectState();
    this.timeline.selectedClipId = null;
    this.timeline.setProject(this.project);
    this.renderTrackHeaders();
    this.renderInspectorPanel();
    this.updateTimecodeReadouts();
    this.compositor.renderFrame(this.project, null, false);
    document.getElementById('modal-welcome')?.classList.add('hidden');
    this.setStatusMessage('Projeto Vazio inicializado e pronto para edição.');
  }

         loadDemoProjectFlow()       {
    this.loadSavedProjectByFile('Neoveg_Broadcast_Demo.veg');
  }

         async saveCurrentProjectFlow()                {
    try {
      const res = await fetch('/api/projects/save', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ project: this.project, fileName: this.project.name }),
      }).then((r) => r.json());
      if (res.ok) {
        if (Array.isArray(res.projects)) this.savedProjects = res.projects;
        this.setStatusMessage(`Projeto salvo em projects/${res.fileName}`);
      }
    } catch (err) {
      this.logErrorToBackend('PROJECT_SAVE', String(err));
    }
  }

         openWelcomeModal()       {
    const modal = document.getElementById('modal-welcome');
    if (!modal) return;
    document.getElementById('modal-advanced-settings')?.classList.add('hidden');
    modal.classList.remove('hidden');
    this.renderWelcomeProjectsList();

    const readSimpleSettings = () => {
      const resVal = (document.getElementById('welcome-res-preset')                     )?.value || '1920x1080';
      const [wStr, hStr] = resVal.split('x');
      const isVertical = wStr === '1080' && hStr === '1920';
      return {
        name:
          (document.getElementById('welcome-project-name')                    )?.value || 'Novo_Projeto_Studio.veg',
        width: Number(wStr || 1920),
        height: Number(hStr || 1080),
        fps: isVertical ? 60 : 29.97,
        pixelAspect: 1.0,
        fieldOrder: 'none (progressive scan)',
        pixelFormat: '8-bit',
        colorSpace: 'Rec.709 Standard',
        motionBlurType: 'Gaussian',
        deinterlaceMethod: 'Blend Fields',
        resampleMode: 'Disable Resample',
        sampleRate: 48000,
        audioBitDepth: 24,
        masterBusMode: 'Stereo',
      };
    };

    // Button: Open Advanced Settings in Separate Window
    const btnOpenAdvWin = document.getElementById('btn-welcome-open-advanced-win');
    if (btnOpenAdvWin) {
      btnOpenAdvWin.onclick = () => {
        this.openAdvancedSettingsWindow();
      };
    }

    // Button: Continue with Empty Project
    const btnEmpty = document.getElementById('btn-welcome-empty-project');
    if (btnEmpty) {
      btnEmpty.onclick = () => {
        modal.classList.add('hidden');
        this.setStatusMessage(`Editando Projeto Vazio (${this.project.name}) — Arraste mídias ou geradores para a Timeline.`);
      };
    }

    // Button: Open Demo Project
    const btnDemo = document.getElementById('btn-welcome-demo-project');
    if (btnDemo) {
      btnDemo.onclick = () => this.loadSavedProjectByFile('Neoveg_Broadcast_Demo.veg');
    }

    // Button: Create & Save New Project (.veg)
    const btnCreate = document.getElementById('btn-welcome-create-project');
    if (btnCreate) {
      btnCreate.onclick = async () => {
        const settings = readSimpleSettings();
        try {
          const res = await fetch('/api/projects/new', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ settings, saveImmediately: true }),
          }).then((r) => r.json());

          if (res.ok && res.project) {
            this.project = res.project;
            if (Array.isArray(res.projects)) this.savedProjects = res.projects;
          } else {
            this.project = this.createEmptyProjectState(settings);
          }
        } catch {
          this.project = this.createEmptyProjectState(settings);
        }

        this.timeline.selectedClipId = null;
        this.timeline.setProject(this.project);
        this.renderTrackHeaders();
        this.renderInspectorPanel();
        this.updateTimecodeReadouts();
        this.compositor.renderFrame(this.project, null, false);
        modal.classList.add('hidden');
        this.setStatusMessage(
          `Novo projeto criado: ${this.project.name} (${this.project.width}x${this.project.height} @ ${this.project.fps}fps)`
        );
      };
    }
  }

         openAdvancedSettingsWindow()       {
    document.getElementById('modal-welcome')?.classList.add('hidden');
    const advModal = document.getElementById('modal-advanced-settings');
    if (!advModal) return;
    advModal.classList.remove('hidden');

    const btnBack = document.getElementById('btn-adv-win-back-welcome');
    if (btnBack) {
      btnBack.onclick = () => {
        advModal.classList.add('hidden');
        this.openWelcomeModal();
      };
    }

    const btnPopoutOs = document.getElementById('btn-popout-advanced-os-win');
    if (btnPopoutOs) {
      btnPopoutOs.onclick = () => {
        advModal.classList.add('hidden');
        const currentTheme = document.documentElement.getAttribute('data-theme') || 'neoveg21-dark';
        const url = new URL(window.location.href);
        url.searchParams.set('popout', 'pane-inspector');
        url.searchParams.set('tab', 'settings');
        url.searchParams.set('theme', currentTheme);
        window.open(
          url.toString(),
          'Neoveg_AdvancedSettings_Window',
          'width=900,height=680,left=140,top=90,resizable=yes,scrollbars=yes'
        );
        this.setStatusMessage('Configurações Avançadas abertas em uma nova janela nativa independente do Windows!');
      };
    }

    const btnApply = document.getElementById('btn-adv-win-apply');
    if (btnApply) {
      btnApply.onclick = () => {
        const resVal = (document.getElementById('adv-win-res')                     )?.value || '1920x1080';
        const [wStr, hStr] = resVal.split('x');
        this.project.width = Number(wStr || 1920);
        this.project.height = Number(hStr || 1080);
        this.project.fps = Number((document.getElementById('adv-win-fps')                     )?.value || 29.97);
        this.project.pixelFormat =
          (document.getElementById('adv-win-pixel-format')                     )?.value || '8-bit';
        this.project.colorSpace =
          (document.getElementById('adv-win-color-space')                     )?.value || 'Rec.709 Standard';
        this.project.resampleMode =
          (document.getElementById('adv-win-resample')                     )?.value || 'Disable Resample';
        this.project.motionBlurType =
          (document.getElementById('adv-win-motion-blur')                     )?.value || 'Gaussian';
        this.project.sampleRate = Number(
          (document.getElementById('adv-win-sample-rate')                     )?.value || 48000
        );
        this.project.audioBitDepth = Number(
          (document.getElementById('adv-win-audio-bit')                     )?.value || 24
        );

        this.updateTimecodeReadouts();
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
        this.broadcastMultiMonitorSync('PROJECT');
        advModal.classList.add('hidden');
        this.setStatusMessage(
          `Configurações Avançadas aplicadas: ${this.project.width}x${this.project.height} @ ${this.project.fps}fps • ${this.project.colorSpace} • ${this.project.sampleRate}Hz`
        );
      };
    }
  }

          renderWelcomeProjectsList()       {
    const listEl = document.getElementById('welcome-projects-list');
    if (!listEl) return;

    if (this.savedProjects.length === 0) {
      listEl.innerHTML = `<div class="empty-note">Carregando projetos salvos (.veg)...</div>`;
      return;
    }

    listEl.innerHTML = this.savedProjects
      .map(
        (p) => `
      <div class="qt-project-card ${p.isDemo ? 'demo-highlight' : ''}">
        <div class="proj-card-top">
          <span class="proj-card-title">${p.name}</span>
          ${p.isDemo ? '<span class="qt-badge-demo">DEMO</span>' : '<span class="qt-badge-user">.VEG</span>'}
        </div>
        <div class="proj-card-desc">${p.description}</div>
        <div class="proj-card-meta">
          <span>${p.width}x${p.height} @ ${p.fps}fps</span>
          <span>• ${p.pixelFormat}</span>
          <span>• ${p.sampleRate}Hz</span>
        </div>
        <div class="proj-card-actions">
          <button class="qt-btn qt-btn-primary" data-open-veg="${p.fileName}">Abrir Projeto</button>
        </div>
      </div>`
      )
      .join('');

    listEl.querySelectorAll('[data-open-veg]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const fname = (btn               ).dataset.openVeg ;
        this.loadSavedProjectByFile(fname);
      });
    });
  }

         applyTheme(themeId        )       {
    document.documentElement.setAttribute('data-theme', themeId);
    document.body.setAttribute('data-theme', themeId);
    const isLight = themeId === 'neoveg4-xp' || themeId === 'neoveg7-classic_nle';
    this.timeline.lightThemeCanvas = isLight;
    this.timeline.render();
    this.setStatusMessage(`Tema visual clássico/NLE aplicado: ${themeId.toUpperCase()}`);
  }

          studioLoop(ts        )       {
    const dt = this.lastFrameTs ? Math.min(0.1, (ts - this.lastFrameTs) / 1000) : 0.016;
    this.lastFrameTs = ts;

    if (this.isPlaying) {
      this.project.playhead += dt * this.shuttleSpeed;
      const maxEnd = Math.max(this.project.loopEnd, this.getProjectDuration());
      if (this.project.loopEnabled && this.project.playhead >= this.project.loopEnd) {
        this.project.playhead = this.project.loopStart;
      } else if (this.project.playhead >= maxEnd) {
        this.project.playhead = 0;
        this.togglePlayback(false);
      } else if (this.project.playhead < 0) {
        this.project.playhead = 0;
      }

      this.updateTimecodeReadouts();
      this.compositor.syncVideos(this.project, true);
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
      this.timeline.render();
    }

    this.updateMasterMeters();
    requestAnimationFrame((nextTs) => this.studioLoop(nextTs));
  }

         getProjectDuration()         {
    let maxT = 10.0;
    for (const tr of this.project.tracks) {
      for (const c of tr.clips) {
        if (c.start + c.duration > maxT) maxT = c.start + c.duration;
      }
    }
    return maxT;
  }

         getSelectedClip()                   {
    if (!this.timeline?.selectedClipId) return null;
    for (const tr of this.project.tracks) {
      const found = tr.clips.find((c) => c.id === this.timeline.selectedClipId);
      if (found) return found;
    }
    return null;
  }

         getSelectedTrack()               {
    if (this.timeline?.selectedTrackId) {
      const found = this.project.tracks.find((t) => t.id === this.timeline.selectedTrackId);
      if (found) return found;
    }
    return this.project.tracks[0] || null;
  }

         togglePlayback(force          )       {
    this.isPlaying = force !== undefined ? force : !this.isPlaying;
    if (!this.isPlaying) {
      this.audioEngine.stopAll();
    }
    this.compositor.syncVideos(this.project, this.isPlaying);
    const playBtn = document.getElementById('btn-transport-play');
    if (playBtn) {
      playBtn.innerHTML = this.isPlaying
        ? `${Neoveg_ICONS.pause}<span>Pause</span>`
        : `${Neoveg_ICONS.play}<span>Play</span>`;
      playBtn.classList.toggle('active', this.isPlaying);
    }
  }

         splitAtPlayhead()       {
    const t = this.project.playhead;
    const selClip = this.getSelectedClip();
    let splitCount = 0;

    for (const track of this.project.tracks) {
      if (track.locked) continue;
      const newClips              = [];
      for (const clip of track.clips) {
        const shouldSplit = selClip ? clip.id === selClip.id : true;
        if (shouldSplit && !clip.locked && t > clip.start + 0.08 && t < clip.start + clip.duration - 0.08) {
          const leftDur = Number((t - clip.start).toFixed(3));
          const rightDur = Number((clip.start + clip.duration - t).toFixed(3));
          const rightClip            = JSON.parse(JSON.stringify(clip));
          rightClip.id = `clip_${Date.now()}_${Math.floor(Math.random() * 1000)}`;
          rightClip.start = Number(t.toFixed(3));
          rightClip.duration = rightDur;
          rightClip.trimIn = Number((clip.trimIn + leftDur * clip.playbackRate).toFixed(3));
          rightClip.fadeIn = 0.04;

          clip.duration = leftDur;
          clip.fadeOut = 0.04;

          newClips.push(clip, rightClip);
          this.timeline.selectedClipId = rightClip.id;
          splitCount++;
        } else {
          newClips.push(clip);
        }
      }
      track.clips = newClips;
    }

    if (splitCount > 0) {
      this.setStatusMessage(`Split ${splitCount} event(s) at ${formatSMPTE(t, this.project.fps)}`);
      this.timeline.render();
      this.renderInspectorPanel();
    } else {
      this.setStatusMessage('Posicione a agulha sobre um evento desbloqueado para dividir (Atalho: S)');
    }
  }

         deleteSelectedClip()       {
    const selId = this.timeline.selectedClipId;
    if (!selId) return;
    for (const track of this.project.tracks) {
      if (track.locked) continue;
      const idx = track.clips.findIndex((c) => c.id === selId);
      if (idx !== -1) {
        const removed = track.clips[idx];
        track.clips.splice(idx, 1);
        if (this.project.rippleEnabled) {
          for (const c of track.clips) {
            if (c.start > removed.start) {
              c.start = Math.max(0, Number((c.start - removed.duration).toFixed(3)));
            }
          }
        }
        this.timeline.selectedClipId = null;
        this.timeline.recalculateAutoCrossfades();
        this.timeline.render();
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, null, false);
        this.setStatusMessage(`Evento removido: "${removed.name}"`);
        return;
      }
    }
  }

         insertMarkerAtPlayhead()       {
    const idx = this.project.markers.length + 1;
    const label = `Marker ${idx}`;
    this.project.markers.push({
      id: `mk_${Date.now()}`,
      time: Number(this.project.playhead.toFixed(3)),
      label,
      color: '#f97316',
    });
    this.project.markers.sort((a, b) => a.time - b.time);
    this.timeline.render();
    this.setStatusMessage(`Marker "${label}" inserido em ${formatSMPTE(this.project.playhead, this.project.fps)}`);
  }

         insertRegionAtLoop()       {
    const idx = this.project.regions.length + 1;
    this.project.regions.push({
      id: `rg_${Date.now()}`,
      start: this.project.loopStart,
      end: this.project.loopEnd,
      label: `Region ${idx}`,
    });
    this.timeline.render();
    this.setStatusMessage(`Region criada [${this.project.loopStart.toFixed(1)}s - ${this.project.loopEnd.toFixed(1)}s]`);
  }

         addTrack(type                   )       {
    const count = this.project.tracks.filter((t) => t.type === type).length + 1;
    const newTrack        = {
      id: `track_${type}_${Date.now()}`,
      name: type === 'video' ? `Video Track ${count}` : `Audio Track ${count}`,
      type,
      color: type === 'video' ? '#2563eb' : '#0891b2',
      level: 1.0,
      pan: 0,
      blendMode: 'sourceAlpha',
      muted: false,
      solo: false,
      locked: false,
      armed: false,
      showEnvelope: false,
      envelopePoints: [
        { time: 0, value: 1.0 },
        { time: 10, value: 1.0 },
      ],
      fx: [],
      clips: [],
    };
    if (type === 'video') {
      this.project.tracks.unshift(newTrack);
    } else {
      this.project.tracks.push(newTrack);
    }
    this.renderTrackHeaders();
    this.timeline.render();
    this.setStatusMessage(`Faixa adicionada: ${newTrack.name}`);
  }

         setEditTool(tool              )       {
    this.timeline.editTool = tool;
    document.querySelectorAll('[data-edit-tool]').forEach((btn) => {
      btn.classList.toggle('active', (btn               ).dataset.editTool === tool);
    });
    this.setStatusMessage(`Ferramenta de Edição ativa: ${tool.toUpperCase()}`);
  }

          bindToolbarAndShortcuts()       {
    // Welcome Hub, Save Project, Advanced Settings Tab & Theme Selector
    document.getElementById('btn-open-welcome')?.addEventListener('click', () => this.openWelcomeModal());
    document.getElementById('btn-save-project')?.addEventListener('click', () => this.saveCurrentProjectFlow());
    document.getElementById('btn-open-settings-tab')?.addEventListener('click', () => this.openSettingsTab());
    document.getElementById('select-ui-theme')?.addEventListener('change', (e) => {
      this.applyTheme((e.target                     ).value);
    });

    // Edit Tools (Normal, Razor, Slip, Rate Stretch)
    document.querySelectorAll('[data-edit-tool]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const t = (btn               ).dataset.editTool                ;
        this.setEditTool(t);
      });
    });

    // Transport buttons
    document.getElementById('btn-transport-play')?.addEventListener('click', () => this.togglePlayback());
    document.getElementById('btn-transport-stop')?.addEventListener('click', () => {
      this.togglePlayback(false);
      this.project.playhead = this.project.loopEnabled ? this.project.loopStart : 0;
      this.updateTimecodeReadouts();
      this.timeline.render();
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
    });
    document.getElementById('btn-transport-start')?.addEventListener('click', () => {
      this.project.playhead = 0;
      this.updateTimecodeReadouts();
      this.timeline.render();
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
    });
    document.getElementById('btn-transport-end')?.addEventListener('click', () => {
      this.project.playhead = this.getProjectDuration();
      this.updateTimecodeReadouts();
      this.timeline.render();
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
    });
    document.getElementById('btn-transport-prev-frame')?.addEventListener('click', () => {
      this.project.playhead = Math.max(0, this.project.playhead - 1 / this.project.fps);
      this.updateTimecodeReadouts();
      this.timeline.render();
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
    });
    document.getElementById('btn-transport-next-frame')?.addEventListener('click', () => {
      this.project.playhead += 1 / this.project.fps;
      this.updateTimecodeReadouts();
      this.timeline.render();
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
    });
    document.getElementById('btn-transport-loop')?.addEventListener('click', (e) => {
      this.project.loopEnabled = !this.project.loopEnabled;
      (e.currentTarget               ).classList.toggle('active', this.project.loopEnabled);
      this.timeline.render();
    });

    // Main Toolbar buttons
    document.getElementById('btn-tool-split')?.addEventListener('click', () => this.splitAtPlayhead());
    document.getElementById('btn-tool-marker')?.addEventListener('click', () => this.insertMarkerAtPlayhead());
    document.getElementById('btn-tool-region')?.addEventListener('click', () => this.insertRegionAtLoop());
    document.getElementById('btn-tool-delete')?.addEventListener('click', () => this.deleteSelectedClip());

    document.getElementById('btn-tool-snap')?.addEventListener('click', (e) => {
      this.project.snapEnabled = !this.project.snapEnabled;
      (e.currentTarget               ).classList.toggle('active', this.project.snapEnabled);
      this.setStatusMessage(`Snap (F8): ${this.project.snapEnabled ? 'ON' : 'OFF'}`);
    });

    document.getElementById('btn-tool-ripple')?.addEventListener('click', (e) => {
      this.project.rippleEnabled = !this.project.rippleEnabled;
      (e.currentTarget               ).classList.toggle('active', this.project.rippleEnabled);
      this.setStatusMessage(`Auto-Ripple (Ctrl+L): ${this.project.rippleEnabled ? 'ON' : 'OFF'}`);
    });

    document.getElementById('btn-tool-xfade')?.addEventListener('click', (e) => {
      this.project.autoCrossfadeEnabled = !this.project.autoCrossfadeEnabled;
      (e.currentTarget               ).classList.toggle('active', this.project.autoCrossfadeEnabled);
      this.timeline.recalculateAutoCrossfades();
      this.timeline.render();
    });

    document.getElementById('btn-add-video-track')?.addEventListener('click', () => this.addTrack('video'));
    document.getElementById('btn-add-audio-track')?.addEventListener('click', () => this.addTrack('audio'));
    document.getElementById('btn-tracks-shrink')?.addEventListener('click', () => {
      this.timeline.adjustAllTrackHeights(-12);
      this.renderTrackHeaders();
      this.setStatusMessage('Altura das faixas de Áudio/Vídeo reduzida.');
    });
    document.getElementById('btn-tracks-expand')?.addEventListener('click', () => {
      this.timeline.adjustAllTrackHeights(12);
      this.renderTrackHeaders();
      this.setStatusMessage('Altura das faixas de Áudio/Vídeo aumentada.');
    });
    document.getElementById('btn-group-av-tracks')?.addEventListener('click', () => {
      this.groupAVTracksFlow();
    });
    document.getElementById('btn-ungroup-tracks')?.addEventListener('click', () => {
      this.ungroupTracksFlow();
    });

    // Preview Header Controls
    document.getElementById('select-preview-quality')?.addEventListener('change', (e) => {
      this.project.previewQuality = (e.target                     ).value                                  ;
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
    });

    document.getElementById('btn-preview-split-bypass')?.addEventListener('click', (e) => {
      this.project.splitScreenBypass = !this.project.splitScreenBypass;
      (e.currentTarget               ).classList.toggle('active', this.project.splitScreenBypass);
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
    });

    document.getElementById('btn-preview-grid')?.addEventListener('click', (e) => {
      this.project.showGrid = !this.project.showGrid;
      (e.currentTarget               ).classList.toggle('active', this.project.showGrid);
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
    });

    document.getElementById('btn-preview-safe')?.addEventListener('click', (e) => {
      this.project.showSafeAreas = !this.project.showSafeAreas;
      (e.currentTarget               ).classList.toggle('active', this.project.showSafeAreas);
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
    });

    document.getElementById('btn-preview-gizmo')?.addEventListener('click', (e) => {
      this.showPanCropGizmo = !this.showPanCropGizmo;
      (e.currentTarget               ).classList.toggle('active', this.showPanCropGizmo);
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
    });

    document.getElementById('btn-preview-snapshot')?.addEventListener('click', () => {
      const canvas = document.getElementById('neoveg-preview-canvas')                     ;
      const a = document.createElement('a');
      a.href = canvas.toDataURL('image/png');
      a.download = `Neoveg_Snapshot_${formatSMPTE(this.project.playhead).replace(/[:;]/g, '-')}.png`;
      a.click();
      this.setStatusMessage('Snapshot PNG salvo.');
    });

    // Left Dock Tabs
    document.querySelectorAll('[data-left-tab]').forEach((btn) => {
      btn.addEventListener('click', () => {
        this.activeLeftTab = (btn               ).dataset.leftTab                             ;
        document.querySelectorAll('[data-left-tab]').forEach((b) => b.classList.remove('active'));
        btn.classList.add('active');
        this.renderLeftDock();
      });
    });

    // Center Dock Tabs
    document.querySelectorAll('[data-center-tab]').forEach((btn) => {
      btn.addEventListener('click', () => {
        this.activeCenterTab = (btn               ).dataset.centerTab                               ;
        this.syncCenterTabButtons();
        this.renderInspectorPanel();
      });
    });

    // Master Fader
    const masterSlider = document.getElementById('master-volume-slider')                           ;
    masterSlider?.addEventListener('input', () => {
      this.project.masterVolume = Number(masterSlider.value);
      const dbLabel = document.getElementById('master-db-readout');
      if (dbLabel) {
        const db = this.project.masterVolume > 0.001 ? 20 * Math.log10(this.project.masterVolume) : -60;
        dbLabel.textContent = `${db >= 0 ? '+' : ''}${db.toFixed(1)} dB`;
      }
    });

    // Modals: Render As, Python Scripting, Neoveg Diagnostics & Welcome
    document.getElementById('btn-open-render-modal')?.addEventListener('click', () => this.openRenderModal());
    document.getElementById('btn-open-script-modal')?.addEventListener('click', () => this.openScriptingModal());
    document.getElementById('btn-open-diagnostics')?.addEventListener('click', () => this.openDiagnosticsModal());
    document.getElementById('btn-open-diagnostics-modal')?.addEventListener('click', () => this.openDiagnosticsModal());

    document.getElementById('btn-float-all-windows')?.addEventListener('click', () => {
      this.dockManager.floatAllPanels();
      this.setStatusMessage('Janelas soltas em modo flutuante livre — arraste pelo topo (⋮⋮) ou até a borda para abrir em outro monitor.');
    });
    document.getElementById('btn-popout-preview-monitor')?.addEventListener('click', () => {
      this.dockManager.popoutPanelToMonitor('pane-preview');
      this.setStatusMessage('Janela Video Preview destacada em janela OS nativa para segundo monitor!');
    });
    document.getElementById('btn-reset-layout')?.addEventListener('click', () => {
      this.dockManager.resetDefaultLayout();
      this.setStatusMessage('Layout acoplado padrão restaurado.');
    });

    document.querySelectorAll('[data-close-modal]').forEach((btn) => {
      btn.addEventListener('click', () => {
        document.querySelectorAll('.neoveg-modal-backdrop').forEach((m) => m.classList.add('hidden'));
      });
    });

    // Import local media file
    const fileInput = document.getElementById('input-import-media')                           ;
    document.getElementById('btn-import-media')?.addEventListener('click', () => fileInput?.click());
    fileInput?.addEventListener('change', () => {
      const files = fileInput.files;
      if (!files || files.length === 0) return;
      for (const file of Array.from(files)) {
        const blobUrl = URL.createObjectURL(file);
        const isAud = file.type.startsWith('audio/') || /\.(wav|mp3|ogg|flac|m4a)$/i.test(file.name);
        const newAsset             = {
          id: `local_${Date.now()}_${Math.floor(Math.random() * 100)}`,
          name: file.name,
          fileName: file.name,
          url: blobUrl,
          type: isAud ? 'audio' : 'video',
          duration: 6.0,
          width: isAud ? 0 : 1280,
          height: isAud ? 0 : 720,
          fps: 30,
          codec: isAud ? 'PCM/AUDIO' : 'H264/VIDEO',
          color: isAud ? '#0891b2' : '#2563eb',
        };
        this.mediaBin.unshift(newAsset);
        this.addMediaAssetToTimeline(newAsset);
      }
      this.renderLeftDock();
    });

    // Global Neoveg & Premiere Keyboard Shortcuts
    window.addEventListener('keydown', (e) => {
      const tag = (e.target               )?.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;

      if (e.code === 'Space') {
        e.preventDefault();
        this.togglePlayback();
      } else if (e.key === 's' || e.key === 'S') {
        if (!e.ctrlKey) {
          e.preventDefault();
          this.splitAtPlayhead();
        }
      } else if (e.key === 'd' || e.key === 'D') {
        this.setEditTool('normal');
      } else if (e.key === 'c' || e.key === 'C') {
        this.setEditTool('razor');
      } else if (e.key === 'y' || e.key === 'Y') {
        this.setEditTool('slip');
      } else if (e.key === 'x' || e.key === 'X') {
        this.setEditTool('rateStretch');
      } else if (e.key === 'm' || e.key === 'M') {
        e.preventDefault();
        this.insertMarkerAtPlayhead();
      } else if (e.key === 'r' || e.key === 'R') {
        e.preventDefault();
        this.insertRegionAtLoop();
      } else if (e.key === 'q' || e.key === 'Q') {
        e.preventDefault();
        this.project.loopEnabled = !this.project.loopEnabled;
        document.getElementById('btn-transport-loop')?.classList.toggle('active', this.project.loopEnabled);
        this.timeline.render();
      } else if (e.key === 'v' || e.key === 'V') {
        e.preventDefault();
        const tr = this.getSelectedTrack();
        if (tr) {
          tr.showEnvelope = !tr.showEnvelope;
          this.renderTrackHeaders();
          this.timeline.render();
        }
      } else if (e.key === 'F8') {
        e.preventDefault();
        this.project.snapEnabled = !this.project.snapEnabled;
        document.getElementById('btn-tool-snap')?.classList.toggle('active', this.project.snapEnabled);
      } else if ((e.ctrlKey || e.metaKey) && (e.key === 'l' || e.key === 'L')) {
        e.preventDefault();
        this.project.rippleEnabled = !this.project.rippleEnabled;
        document.getElementById('btn-tool-ripple')?.classList.toggle('active', this.project.rippleEnabled);
      } else if (e.key === 'Delete' || e.key === 'Backspace') {
        e.preventDefault();
        this.deleteSelectedClip();
      } else if (e.key === 'ArrowLeft') {
        e.preventDefault();
        this.project.playhead = Math.max(0, this.project.playhead - 1 / this.project.fps);
        this.updateTimecodeReadouts();
        this.timeline.render();
        this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
      } else if (e.key === 'ArrowRight') {
        e.preventDefault();
        this.project.playhead += 1 / this.project.fps;
        this.updateTimecodeReadouts();
        this.timeline.render();
        this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
      }
    });

    window.addEventListener('click', () => {
      document.getElementById('clip-context-menu')?.classList.add('hidden');
    });
  }

         addMediaAssetToTimeline(asset            , dropTime         , targetTrackOverride               )       {
    const targetType = asset.type === 'audio' ? 'audio' : 'video';
    let targetTrack                    = undefined;

    if (targetTrackOverride && !targetTrackOverride.locked && targetTrackOverride.type === targetType) {
      targetTrack = targetTrackOverride;
    }
    if (!targetTrack) {
      targetTrack = this.project.tracks.find(
        (t) => t.id === this.timeline.selectedTrackId && t.type === targetType && !t.locked
      );
    }
    if (!targetTrack) {
      targetTrack = this.project.tracks.find((t) => t.type === targetType && !t.locked);
    }
    if (!targetTrack) return;

    const startTime = dropTime !== undefined ? Math.max(0, Number(dropTime.toFixed(3))) : Number(this.project.playhead.toFixed(3));

    const newClip            = {
      id: `clip_${Date.now()}_${Math.floor(Math.random() * 999)}`,
      name: asset.name,
      mediaId: asset.id,
      mediaFile: asset.fileName,
      mediaUrl: asset.url,
      mediaType: asset.type,
      generatorType: asset.generatorType || (asset.type === 'video' ? 'smpteBars' : undefined),
      color: asset.color || (targetType === 'video' ? '#2563eb' : '#0891b2'),
      start: startTime,
      duration: asset.duration || 5.0,
      trimIn: 0,
      fadeIn: 0.25,
      fadeOut: 0.25,
      fadeInCurve: 'smooth',
      fadeOutCurve: 'smooth',
      gain: 1.0,
      playbackRate: 1.0,
      muted: false,
      locked: false,
      panCrop: {
        x: 0,
        y: 0,
        scale: 1.0,
        rotation: 0,
        flipH: false,
        flipV: false,
        lockAspect: true,
        sizeAboutCenter: true,
        keyframes: [{ time: 0, x: 0, y: 0, scale: 1.0, rotation: 0 }],
      },
      fx: [],
      peaks: asset.peaks,
    };

    targetTrack.clips.push(newClip);
    this.timeline.selectedClipId = newClip.id;
    this.timeline.selectedTrackId = targetTrack.id;
    this.timeline.recalculateAutoCrossfades();
    this.timeline.render();
    this.renderTrackHeaders();
    this.renderInspectorPanel();
    this.compositor.renderFrame(this.project, newClip, this.showPanCropGizmo);
    this.broadcastMultiMonitorSync('PROJECT');
    this.setStatusMessage(
      `Inserido "${newClip.name}" na faixa ${targetTrack.name} em ${formatSMPTE(startTime, this.project.fps)}`
    );
  }

         handleTimelineDrop(dropTime        , targetTrack              , event           )       {
    // 1. Check if external OS files were dragged from Windows Explorer directly onto the Timeline
    const files = event.dataTransfer?.files;
    if (files && files.length > 0 && !this.activeDragPayload) {
      let currentOffset = dropTime;
      for (const file of Array.from(files)) {
        const blobUrl = URL.createObjectURL(file);
        const isAud = file.type.startsWith('audio/') || /\.(wav|mp3|ogg|flac|m4a)$/i.test(file.name);
        const newAsset             = {
          id: `local_${Date.now()}_${Math.floor(Math.random() * 1000)}`,
          name: file.name,
          fileName: file.name,
          url: blobUrl,
          type: isAud ? 'audio' : 'video',
          duration: 6.0,
          width: isAud ? 0 : 1280,
          height: isAud ? 0 : 720,
          fps: 30,
          codec: isAud ? 'PCM/AUDIO' : 'H264/VIDEO',
          color: isAud ? '#0891b2' : '#2563eb',
        };
        this.mediaBin.unshift(newAsset);
        this.addMediaAssetToTimeline(newAsset, currentOffset, targetTrack);
        currentOffset += newAsset.duration;
      }
      this.renderLeftDock();
      return;
    }

    // 2. Internal Studio Drag Payload (Media, Generators, Video FX, LADSPA Audio FX, Transitions)
    let payload                             = this.activeDragPayload;
    if (!payload && event.dataTransfer) {
      const raw = event.dataTransfer.getData('application/x-neoveg-drag');
      if (raw) {
        try {
          payload = JSON.parse(raw)                       ;
        } catch {
          payload = null;
        }
      }
    }
    this.activeDragPayload = null;
    this.multiMonitorBus?.postMessage({ type: 'SYNC_DRAG_PAYLOAD', payload: null });
    if (!payload) return;

    if ((payload.kind === 'media' || payload.kind === 'generator') && payload.asset) {
      this.addMediaAssetToTimeline(payload.asset, dropTime, targetTrack);
      return;
    }

    // Find clip under drop coordinates (or fall back to selected clip)
    let targetClip                   = null;
    if (targetTrack) {
      targetClip =
        targetTrack.clips.find((c) => dropTime >= c.start && dropTime <= c.start + c.duration) ||
        targetTrack.clips[targetTrack.clips.length - 1] ||
        null;
    }
    if (!targetClip) {
      targetClip = this.getSelectedClip();
    }

    if ((payload.kind === 'fx' || payload.kind === 'audiofx') && payload.fxType) {
      if (!targetClip) {
        // Automatically create a demo clip at dropTime if dropped onto an empty timeline!
        const fallbackAsset             =
          payload.fxType === 'corridorKey'
            ? {
                id: 'gen_ck_drop',
                name: 'CorridorKey Green Screen Subject',
                type: 'generator',
                generatorType: 'greenScreenActor',
                duration: 6.0,
                width: 1280,
                height: 720,
                fps: 30,
                codec: 'CORRIDORKEY',
                color: '#16a34a',
              }
            : this.mediaBin[0] || {
                id: 'gen_auto_drop',
                name: 'Neoveg Synthwave Horizon Scene',
                type: 'generator',
                generatorType: 'cyberGrid',
                duration: 5.0,
                width: 1280,
                height: 720,
                fps: 30,
                codec: 'GENERATOR',
                color: '#2563eb',
              };
        this.addMediaAssetToTimeline(fallbackAsset, dropTime, targetTrack);
        targetClip = this.getSelectedClip();
      }
      if (targetClip) {
        targetClip.fx.push({
          id: `fx_${Date.now()}`,
          type: payload.fxType,
          name: payload.fxName || payload.name,
          enabled: true,
          params: { ...(payload.fxDefaults || {}) },
        });
        this.timeline.selectedClipId = targetClip.id;
        this.activeCenterTab = 'fxchain';
        this.syncCenterTabButtons();
        this.timeline.render();
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, targetClip, this.showPanCropGizmo);
        this.broadcastMultiMonitorSync('PROJECT');
        this.setStatusMessage(`Efeito "${payload.name}" solto sobre o clipe "${targetClip.name}"`);
      }
      return;
    }

    if (payload.kind === 'transition' && targetClip) {
      targetClip.fadeIn = Math.max(0.8, targetClip.fadeIn);
      targetClip.fadeOut = Math.max(0.8, targetClip.fadeOut);
      if (payload.transitionCurve) {
        targetClip.fadeInCurve = payload.transitionCurve;
        targetClip.fadeOutCurve = payload.transitionCurve;
      }
      if (payload.transitionType) {
        targetClip.transitionType = payload.transitionType;
      }
      this.timeline.selectedClipId = targetClip.id;
      this.timeline.render();
      this.renderInspectorPanel();
      this.compositor.renderFrame(this.project, targetClip, this.showPanCropGizmo);
      this.broadcastMultiMonitorSync('PROJECT');
      this.setStatusMessage(`Transição "${payload.name}" solta sobre "${targetClip.name}"`);
    }
  }

          bindDraggableCard(cardEl             , payload                     )       {
    cardEl.setAttribute('draggable', 'true');
    cardEl.addEventListener('dragstart', (ev) => {
      this.activeDragPayload = payload;
      this.multiMonitorBus?.postMessage({ type: 'SYNC_DRAG_PAYLOAD', payload });
      cardEl.classList.add('is-dragging-card');
      if (ev.dataTransfer) {
        ev.dataTransfer.effectAllowed = 'copy';
        ev.dataTransfer.setData('application/x-neoveg-drag', JSON.stringify(payload));
        ev.dataTransfer.setData('text/plain', payload.name);
      }
    });
    cardEl.addEventListener('dragend', () => {
      cardEl.classList.remove('is-dragging-card');
      this.activeDragPayload = null;
      this.multiMonitorBus?.postMessage({ type: 'SYNC_DRAG_PAYLOAD', payload: null });
    });
  }

         renderLeftDock()       {
    const container = document.getElementById('left-dock-body');
    if (!container) return;

    if (this.activeLeftTab === 'media') {
      const cards = this.mediaBin
        .map(
          (m) => `
        <div class="media-card" draggable="true" data-media-id="${m.id}" title="Arraste diretamente para qualquer faixa na Timeline ou clique em + Track">
          <div class="media-badge ${m.type}">${m.type === 'audio' ? 'WAV' : 'MP4'}</div>
          <div class="media-info">
            <div class="media-title">${m.name}</div>
            <div class="media-meta">
              ${m.type === 'audio' ? `${m.sampleRate || 48000}Hz • Peak ${m.peakDb ?? -1.2} dB` : `${m.width}x${m.height} • ${m.fps}fps`}
              • ${m.duration.toFixed(1)}s • <span class="drag-hint-pill">⋮⋮ Arraste p/ Timeline</span>
            </div>
          </div>
          <button class="qt-btn qt-btn-primary" data-insert-media="${m.id}">+ Track</button>
        </div>`
        )
        .join('');

      container.innerHTML = `
        <div class="dock-section-header">
          <span>Project Media Bin (${this.mediaBin.length}) — Arraste para a Timeline</span>
          <button class="qt-btn" id="btn-quick-load-demo">Carregar Demo</button>
        </div>
        <div class="media-grid">${cards || '<div class="empty-note">Carregando mídias...</div>'}</div>
      `;

      document.getElementById('btn-quick-load-demo')?.addEventListener('click', () => {
        this.loadSavedProjectByFile('Neoveg_Broadcast_Demo.veg');
      });

      container.querySelectorAll('.media-card[data-media-id]').forEach((card) => {
        const id = (card               ).dataset.mediaId;
        const asset = this.mediaBin.find((x) => x.id === id);
        if (asset) {
          this.bindDraggableCard(card               , {
            kind: 'media',
            id: asset.id,
            name: asset.name,
            mediaType: asset.type,
            duration: asset.duration || 5.0,
            color: asset.color || (asset.type === 'audio' ? '#0891b2' : '#2563eb'),
            asset,
          });
        }
      });

      container.querySelectorAll('[data-insert-media]').forEach((btn) => {
        btn.addEventListener('click', () => {
          const id = (btn               ).dataset.insertMedia;
          const asset = this.mediaBin.find((x) => x.id === id);
          if (asset) this.addMediaAssetToTimeline(asset);
        });
      });
      return;
    }

    if (this.activeLeftTab === 'transitions') {
      const transitions         
                     
                             
                                           
                     
         = [
        { name: 'Neoveg Crossfade (Smooth S-Curve)', curve: 'smooth', trans: 'crossfade', desc: 'Dissolução clássica igual-potência em S' },
        { name: 'Neoveg Crossfade (Fast Attack)', curve: 'fast', trans: 'crossfade', desc: 'Curva logarítmica de ataque rápido' },
        { name: 'Neoveg Crossfade (Slow Exponential)', curve: 'slow', trans: 'crossfade', desc: 'Curva exponencial progressiva' },
        { name: 'Neoveg Linear Wipe Left-to-Right', curve: 'linear', trans: 'wipeLeft', desc: 'Cortina horizontal broadcast' },
        { name: 'Neoveg Iris Circle Reveal', curve: 'smooth', trans: 'irisCircle', desc: 'Máscara circular expansiva' },
        { name: 'Neoveg Flash White Cut', curve: 'fast', trans: 'flashWhite', desc: 'Flash branco de exposição' },
      ];

      container.innerHTML = `
        <div class="dock-section-header">
          <span>Neoveg Transitions & 5 Fade Curves (Arraste sobre um Clipe)</span>
        </div>
        <div class="media-grid">
          ${transitions
            .map(
              (t, idx) => `
            <div class="media-card" draggable="true" data-trans-card="${idx}" title="Arraste para cima de um clipe na Timeline">
              <div class="media-badge trans">X-FADE</div>
              <div class="media-info">
                <div class="media-title">${t.name}</div>
                <div class="media-meta">${t.desc} • <span class="drag-hint-pill">⋮⋮ Arraste</span></div>
              </div>
              <button class="qt-btn qt-btn-primary" data-apply-trans="${idx}">Aplicar</button>
            </div>`
            )
            .join('')}
        </div>
      `;

      container.querySelectorAll('[data-trans-card]').forEach((card) => {
        const idx = Number((card               ).dataset.transCard);
        const item = transitions[idx];
        this.bindDraggableCard(card               , {
          kind: 'transition',
          id: `trans_${idx}`,
          name: item.name,
          mediaType: 'video',
          duration: 1.5,
          color: '#f59e0b',
          transitionCurve: item.curve,
          transitionType: item.trans,
        });
      });

      container.querySelectorAll('[data-apply-trans]').forEach((btn) => {
        btn.addEventListener('click', () => {
          const idx = Number((btn               ).dataset.applyTrans);
          const item = transitions[idx];
          const clip = this.getSelectedClip();
          if (!clip) {
            this.setStatusMessage('Selecione um evento na timeline primeiro.');
            return;
          }
          clip.fadeIn = Math.max(0.8, clip.fadeIn);
          clip.fadeOut = Math.max(0.8, clip.fadeOut);
          clip.fadeInCurve = item.curve;
          clip.fadeOutCurve = item.curve;
          clip.transitionType = item.trans;
          this.timeline.render();
          this.renderInspectorPanel();
          this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
        });
      });
      return;
    }

    if (this.activeLeftTab === 'fx') {
      // Video FX + Frei0r Plugins + CorridorKey AI with Human Face Portrait Demo Thumbnails!
      const fxCatalog         
                          
                     
                      
                     
                                         
         = [
        {
          type: 'corridorKey',
          name: 'CorridorKey AI Green Screen Unmixer',
          badge: 'CORRIDORKEY',
          desc: 'nikopueringer/CorridorKey: Desmixagem de cor & alpha linear sem spill verde',
          defaults: { similarity: 0.28, despill: 0.9, blend: 0.08 },
        },
        {
          type: 'colorGrade',
          name: 'Neoveg / Resolve 3-Wheel Color Grade',
          badge: 'COLOR',
          desc: 'Lift / Gamma / Gain Teal & Orange cinematográfico',
          defaults: { liftR: -0.05, liftB: 0.08, gainR: 1.14, gainB: 0.92, saturation: 1.22 },
        },
        {
          type: 'brightnessContrast',
          name: 'Neoveg Brightness & Contrast',
          badge: 'Neoveg FX',
          desc: 'Exposição, contraste e saturação broadcast',
          defaults: { brightness: 0.08, contrast: 1.2, saturation: 1.15 },
        },
        {
          type: 'frei0r.glow',
          name: 'Frei0r Glow (Optical Bloom)',
          badge: 'FREI0R',
          desc: 'Plugin Frei0r de difusão luminosa em altas-luzes',
          defaults: { blur: 0.45, intensity: 0.65 },
        },
        {
          type: 'frei0r.cartoon',
          name: 'Frei0r Cartoon (Cel-Shaded)',
          badge: 'FREI0R',
          desc: 'Plugin Frei0r de quantização tonal e contorno',
          defaults: { edge: 0.7, quantize: 0.5 },
        },
        {
          type: 'frei0r.pixeliz0r',
          name: 'Frei0r Pixeliz0r (Mosaic)',
          badge: 'FREI0R',
          desc: 'Plugin Frei0r de mosaico pixelado',
          defaults: { blockSize: 8.0 },
        },
        {
          type: 'frei0r.sobel',
          name: 'Frei0r Sobel (Edge Contour)',
          badge: 'FREI0R',
          desc: 'Detector espacial de bordas Sobel 2D',
          defaults: { threshold: 0.5 },
        },
        {
          type: 'frei0r.coloradj_RGB',
          name: 'Frei0r RGB Channel Balance',
          badge: 'FREI0R',
          desc: 'Multiplicador R/G/B de calibração de câmera',
          defaults: { r: 1.15, g: 1.0, b: 0.88 },
        },
        {
          type: 'vignette',
          name: 'Neoveg Soft Lens Vignette',
          badge: 'Neoveg FX',
          desc: 'Escurecimento óptico radial de bordas',
          defaults: { strength: 0.65 },
        },
        {
          type: 'blur',
          name: 'Neoveg Gaussian Blur',
          badge: 'Neoveg FX',
          desc: 'Desfoque gaussiano de profundidade de campo',
          defaults: { radius: 4.5 },
        },
        {
          type: 'vhsGlitch',
          name: 'Neoveg TV Simulator / VHS',
          badge: 'Neoveg FX',
          desc: 'Scanlines analógicas e desvio cromático',
          defaults: { intensity: 0.6 },
        },
        {
          type: 'sepia',
          name: 'Neoveg Archival Sepia Tone',
          badge: 'Neoveg FX',
          desc: 'Viragem sépia fotográfica clássica',
          defaults: { amount: 0.85 },
        },
        {
          type: 'invert',
          name: 'Neoveg Negative Invert',
          badge: 'Neoveg FX',
          desc: 'Inversão RGB completa',
          defaults: { amount: 1.0 },
        },
      ];

      container.innerHTML = `
        <div class="dock-section-header">
          <span>Video FX • Frei0r • CorridorKey (Demo Rosto Humano — Arraste p/ Clipe)</span>
        </div>
        <div class="media-grid">
          ${fxCatalog
            .map(
              (fx, idx) => `
            <div class="media-card fx-thumb-card" draggable="true" data-fx-card="${idx}" title="Arraste este efeito diretamente para um clipe na Timeline">
              <canvas class="fx-face-thumb" width="68" height="50" data-face-fx="${fx.type}"></canvas>
              <div class="media-info">
                <div class="fx-card-title-row">
                  <span class="qt-tag-mini">${fx.badge}</span>
                  <span class="media-title">${fx.name}</span>
                </div>
                <div class="media-meta">${fx.desc}</div>
              </div>
              <button class="qt-btn qt-btn-primary" data-add-fx="${idx}">+ FX</button>
            </div>`
            )
            .join('')}
        </div>
      `;

      // Render human face portrait demonstration on every thumbnail canvas!
      container.querySelectorAll('canvas[data-face-fx]').forEach((canv) => {
        const fxType = (canv               ).dataset.faceFx || '';
        renderHumanFaceEffectThumbnail(canv                     , fxType);
      });

      container.querySelectorAll('[data-fx-card]').forEach((card) => {
        const idx = Number((card               ).dataset.fxCard);
        const item = fxCatalog[idx];
        this.bindDraggableCard(card               , {
          kind: 'fx',
          id: `fx_${item.type}`,
          name: item.name,
          mediaType: 'video',
          duration: 5.0,
          color: '#10b981',
          fxType: item.type,
          fxName: item.name,
          fxDefaults: item.defaults,
        });
      });

      container.querySelectorAll('[data-add-fx]').forEach((btn) => {
        btn.addEventListener('click', () => {
          const idx = Number((btn               ).dataset.addFx);
          const item = fxCatalog[idx];
          let clip = this.getSelectedClip();
          if (!clip) {
            const genAsset             =
              item.type === 'corridorKey'
                ? {
                    id: 'gen_ck_actor',
                    name: 'CorridorKey Green Screen Actor',
                    type: 'generator',
                    generatorType: 'greenScreenActor',
                    duration: 6.0,
                    width: 1280,
                    height: 720,
                    fps: 29.97,
                    codec: 'GENERATOR',
                    color: '#16a34a',
                  }
                : this.mediaBin[0] || {
                    id: 'gen_cyber_auto',
                    name: 'Cyber Horizon Scene',
                    type: 'generator',
                    generatorType: 'cyberGrid',
                    duration: 5.0,
                    width: 1280,
                    height: 720,
                    fps: 29.97,
                    codec: 'GENERATOR',
                    color: '#2563eb',
                  };
            this.addMediaAssetToTimeline(genAsset);
            clip = this.getSelectedClip();
          }
          if (!clip) return;
          clip.fx.push({
            id: `fx_${Date.now()}`,
            type: item.type,
            name: item.name,
            enabled: true,
            params: { ...item.defaults },
          });
          this.activeCenterTab = 'fxchain';
          this.syncCenterTabButtons();
          this.renderInspectorPanel();
          this.timeline.render();
          this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
          this.setStatusMessage(`Efeito "${item.name}" adicionado a ${clip.name}`);
        });
      });
      return;
    }

    if (this.activeLeftTab === 'audiofx') {
      const ladspaList         
                          
                     
                     
                                         
         = [
        {
          type: 'ladspa.sc4_1882',
          name: 'LADSPA SC4 Stereo Compressor (1882)',
          desc: 'Compressor dinâmico RMS/Peak para voz e master',
          defaults: { thresholdDb: -16, ratio: 4.0, makeupDb: 3.0 },
        },
        {
          type: 'ladspa.fast_lookahead_limiter_1913',
          name: 'LADSPA Fast Lookahead Limiter (1913)',
          desc: 'Limiter brickwall True-Peak (-1.0 dBTP)',
          defaults: { limitDb: -1.0, inputGainDb: 2.5 },
        },
        {
          type: 'ladspa.mbeq_1197',
          name: 'LADSPA Multiband Equalizer (1197)',
          desc: 'Equalizador harmônico de 15 bandas',
          defaults: { lowBoostDb: 3.0, highAirDb: 2.5 },
        },
        {
          type: 'ladspa.plate_1423',
          name: 'LADSPA Studio Plate Reverb (1423)',
          desc: 'Reverberação acústica de placa metálica',
          defaults: { reverbTime: 1.8, wetMix: 0.25 },
        },
        {
          type: 'ladspa.gate_1410',
          name: 'LADSPA Noise Gate (1410)',
          desc: 'Corte automático de ruído de fundo entre falas',
          defaults: { thresholdDb: -38.0 },
        },
      ];

      container.innerHTML = `
        <div class="dock-section-header">
          <span>LADSPA Audio DSP • Fairlight LUFS (Arraste p/ Clipe de Áudio)</span>
        </div>
        <div class="media-grid">
          ${ladspaList
            .map(
              (af, idx) => `
            <div class="media-card" draggable="true" data-ladspa-card="${idx}" title="Arraste este plugin LADSPA para um clipe de áudio na Timeline">
              <div class="media-badge audio">LADSPA</div>
              <div class="media-info">
                <div class="media-title">${af.name}</div>
                <div class="media-meta">${af.desc} • <span class="drag-hint-pill">⋮⋮ Arraste</span></div>
              </div>
              <button class="qt-btn qt-btn-primary" data-add-ladspa="${idx}">+ Audio FX</button>
            </div>`
            )
            .join('')}
        </div>
      `;

      container.querySelectorAll('[data-ladspa-card]').forEach((card) => {
        const idx = Number((card               ).dataset.ladspaCard);
        const item = ladspaList[idx];
        this.bindDraggableCard(card               , {
          kind: 'audiofx',
          id: `ladspa_${item.type}`,
          name: item.name,
          mediaType: 'audio',
          duration: 5.0,
          color: '#0891b2',
          fxType: item.type,
          fxName: item.name,
          fxDefaults: item.defaults,
        });
      });

      container.querySelectorAll('[data-add-ladspa]').forEach((btn) => {
        btn.addEventListener('click', () => {
          const idx = Number((btn               ).dataset.addLadspa);
          const item = ladspaList[idx];
          const clip = this.getSelectedClip();
          if (!clip) {
            this.setStatusMessage('Selecione um evento de áudio ou vídeo na timeline.');
            return;
          }
          clip.fx.push({
            id: `ladspa_${Date.now()}`,
            type: item.type,
            name: item.name,
            enabled: true,
            params: { ...item.defaults },
          });
          this.activeCenterTab = 'fxchain';
          this.syncCenterTabButtons();
          this.renderInspectorPanel();
          this.setStatusMessage(`Plugin LADSPA "${item.name}" inserido em ${clip.name}`);
        });
      });
      return;
    }

    if (this.activeLeftTab === 'generators') {
      const gens               = [
        {
          id: 'gen_ck_greenscreen',
          name: 'CorridorKey Green Screen Subject (Ator em Fundo Verde)',
          type: 'generator',
          generatorType: 'greenScreenActor',
          duration: 6.0,
          width: 1280,
          height: 720,
          fps: 30,
          codec: 'CORRIDORKEY',
          color: '#16a34a',
        },
        {
          id: 'gen_title_card',
          name: 'Neoveg Titles & Text Overlay',
          type: 'title',
          generatorType: 'titleText',
          duration: 4.5,
          width: 1280,
          height: 720,
          fps: 30,
          codec: 'GENERATOR',
          color: '#6366f1',
        },
        {
          id: 'gen_cyber_scene',
          name: 'Neoveg Synthwave Horizon Grid',
          type: 'generator',
          generatorType: 'cyberGrid',
          duration: 5.0,
          width: 1280,
          height: 720,
          fps: 30,
          codec: 'GENERATOR',
          color: '#ec4899',
        },
        {
          id: 'gen_credits_roll',
          name: 'Neoveg Scrolling Credit Roll',
          type: 'generator',
          generatorType: 'creditRoll',
          duration: 6.0,
          width: 1280,
          height: 720,
          fps: 30,
          codec: 'GENERATOR',
          color: '#0284c7',
        },
        {
          id: 'gen_smpte_bars',
          name: 'Neoveg SMPTE HD Color Bars',
          type: 'generator',
          generatorType: 'smpteBars',
          duration: 4.0,
          width: 1280,
          height: 720,
          fps: 30,
          codec: 'GENERATOR',
          color: '#2563eb',
        },
      ];

      container.innerHTML = `
        <div class="dock-section-header">
          <span>Neoveg Media Generators (Arraste diretamente para a Timeline)</span>
        </div>
        <div class="media-grid">
          ${gens
            .map(
              (g, idx) => `
            <div class="media-card" draggable="true" data-gen-card="${idx}" title="Arraste este Gerador diretamente para qualquer faixa na Timeline">
              <div class="media-badge gen">GEN</div>
              <div class="media-info">
                <div class="media-title">${g.name}</div>
                <div class="media-meta">Gerador Procedural 60fps • ${g.duration}s • <span class="drag-hint-pill">⋮⋮ Arraste p/ Timeline</span></div>
              </div>
              <button class="qt-btn qt-btn-primary" data-insert-gen="${idx}">+ Inserir</button>
            </div>`
            )
            .join('')}
        </div>
      `;

      container.querySelectorAll('[data-gen-card]').forEach((card) => {
        const idx = Number((card               ).dataset.genCard);
        const genAsset = gens[idx];
        this.bindDraggableCard(card               , {
          kind: 'generator',
          id: genAsset.id,
          name: genAsset.name,
          mediaType: 'generator',
          duration: genAsset.duration,
          color: genAsset.color,
          asset: genAsset,
        });
      });

      container.querySelectorAll('[data-insert-gen]').forEach((btn) => {
        btn.addEventListener('click', () => {
          const idx = Number((btn               ).dataset.insertGen);
          this.addMediaAssetToTimeline(gens[idx]);
        });
      });
    }
  }

         async renameTrackById(trackId        , newName        )                {
    const clean = newName.trim().slice(0, 64) || 'Untitled Track';
    const tr = this.project.tracks.find((t) => t.id === trackId);
    if (!tr) return;
    tr.name = clean;
    this.timeline.render();
    this.broadcastMultiMonitorSync('PROJECT');
    this.setStatusMessage(`Faixa renomeada para: "${clean}"`);

    try {
      const res = await fetch('/api/tracks/rename', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ project: this.project, trackId, newName: clean }),
      }).then((r) => r.json());
      if (res.ok && res.project) {
        this.project = res.project;
        this.timeline.setProject(this.project);
      }
    } catch {
      // Fallback already applied locally
    }
  }

         async groupAVTracksFlow(specificTrackIds           , customGroupName         )                {
    const palette = ['#f59e0b', '#10b981', '#a855f7', '#ec4899', '#38bdf8'];
    let targetIds = specificTrackIds || [];

    if (targetIds.length === 0) {
      // Pair ungrouped video + audio tracks, or group all tracks if <= 2
      const ungroupedVideo = this.project.tracks.find((t) => t.type === 'video' && !t.groupId);
      const ungroupedAudio = this.project.tracks.find((t) => t.type === 'audio' && !t.groupId);
      if (ungroupedVideo && ungroupedAudio) {
        targetIds = [ungroupedVideo.id, ungroupedAudio.id];
      } else {
        targetIds = this.project.tracks.map((t) => t.id);
      }
    }

    const existingGroups = new Set(this.project.tracks.map((t) => t.groupId).filter(Boolean));
    const groupIdx = existingGroups.size + 1;
    const groupId = `grp_av_${groupIdx}`;
    const groupName = customGroupName || `Grupo V+A ${groupIdx}`;
    const groupColor = palette[(groupIdx - 1) % palette.length];

    for (const tr of this.project.tracks) {
      if (targetIds.includes(tr.id)) {
        tr.groupId = groupId;
        tr.groupName = groupName;
        tr.groupColor = groupColor;
      }
    }

    this.renderTrackHeaders();
    this.timeline.render();
    this.broadcastMultiMonitorSync('PROJECT');
    this.setStatusMessage(`Faixas de Vídeo e Áudio agrupadas em "${groupName}" (Mute/Solo sincronizados).`);

    try {
      const res = await fetch('/api/tracks/group', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          action: 'group',
          project: this.project,
          trackIds: targetIds,
          groupId,
          groupName,
          groupColor,
        }),
      }).then((r) => r.json());
      if (res.ok && res.project) {
        this.project = res.project;
        this.timeline.setProject(this.project);
        this.renderTrackHeaders();
      }
    } catch {
      // Local state already updated
    }
  }

         async ungroupTracksFlow(trackId         )                {
    const targetId = trackId || this.timeline.selectedTrackId || undefined;
    const targetTrack = targetId ? this.project.tracks.find((t) => t.id === targetId) : undefined;

    if (targetTrack && targetTrack.groupId) {
      const gid = targetTrack.groupId;
      for (const tr of this.project.tracks) {
        if (tr.groupId === gid) {
          delete tr.groupId;
          delete tr.groupName;
          delete tr.groupColor;
        }
      }
      this.setStatusMessage(`Grupo de faixas desfeito para "${targetTrack.name}".`);
    } else {
      for (const tr of this.project.tracks) {
        delete tr.groupId;
        delete tr.groupName;
        delete tr.groupColor;
      }
      this.setStatusMessage('Todas as faixas foram desagrupadas.');
    }

    this.renderTrackHeaders();
    this.timeline.render();
    this.broadcastMultiMonitorSync('PROJECT');

    try {
      await fetch('/api/tracks/group', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          action: 'ungroup',
          project: this.project,
          trackId: targetTrack?.id,
        }),
      });
    } catch {
      // Local state already updated
    }
  }

         renderTrackHeaders()       {
    const container = document.getElementById('neoveg-track-headers');
    if (!container) return;

    container.innerHTML = this.project.tracks
      .map((track, idx) => {
        const isSelected = this.timeline?.selectedTrackId === track.id;
        const levelPct = Math.round(track.level * 100);
        const th = this.timeline ? this.timeline.getTrackHeight(track) : track.height || 74;
        const hasGroup = Boolean(track.groupId);
        const grpColor = track.groupColor || '#f59e0b';
        const grpBorderStyle = hasGroup ? `border-left: 4px solid ${grpColor};` : '';
        return `
        <div class="neoveg-track-header ${track.type} ${isSelected ? 'selected' : ''} ${hasGroup ? 'grouped-track' : ''}" data-track-id="${track.id}" style="height:${th}px; ${grpBorderStyle}">
          <div class="track-hdr-top">
            <span class="track-num-badge" style="background:${track.color}" title="${track.type === 'video' ? 'Faixa de Vídeo' : 'Faixa de Áudio'}">${idx + 1}</span>
            <input class="track-name-input" data-track-name="${track.id}" value="${track.name}" title="Clique para renomear esta faixa de ${track.type === 'video' ? 'Vídeo' : 'Áudio'} (Enter para confirmar)" />
            <button class="track-btn" data-track-rename="${track.id}" title="Renomear Faixa (${track.name})">${Neoveg_ICONS.renameEdit}</button>
            <button class="track-btn ${hasGroup ? 'active' : ''}" data-track-group="${track.id}" style="${hasGroup ? `border-color:${grpColor};color:${grpColor};` : ''}" title="${hasGroup ? `Agrupada em: ${track.groupName} (Clique para desagrupar)` : 'Agrupar esta faixa com Vídeo+Áudio (G)'}">G</button>
            <button class="track-btn track-h-btn" data-track-h-dec="${track.id}" title="Diminuir altura desta faixa (-)">${Neoveg_ICONS.minus}</button>
            <button class="track-btn track-h-btn" data-track-h-inc="${track.id}" title="Aumentar altura desta faixa (+)">${Neoveg_ICONS.plus}</button>
            <button class="track-btn ${track.showEnvelope ? 'active' : ''}" data-track-env="${track.id}" title="Automation Envelope (V)">V</button>
            <button class="track-btn mute ${track.muted ? 'active' : ''}" data-track-mute="${track.id}" title="Mute Track (Sincronizado se agrupada)">M</button>
            <button class="track-btn solo ${track.solo ? 'active' : ''}" data-track-solo="${track.id}" title="Solo Track (Sincronizado se agrupada)">S</button>
          </div>
          <div class="track-hdr-bottom">
            ${
              hasGroup
                ? `<span class="track-group-pill" style="border-color:${grpColor};color:${grpColor}" title="Grupo de Faixas Ativo">${track.groupName || 'V+A'}</span>`
                : ''
            }
            <label class="track-slider-lbl">
              <span>${track.type === 'video' ? 'Opac' : 'Vol'}</span>
              <input type="range" min="0" max="${track.type === 'video' ? '1' : '1.4'}" step="0.02" value="${track.level}" data-track-level="${track.id}" />
              <span class="track-val">${levelPct}%</span>
            </label>
            ${
              track.type === 'video'
                ? `<select class="track-blend-select" data-track-blend="${track.id}" title="Neoveg Compositing Mode">
                    <option value="sourceAlpha" ${track.blendMode === 'sourceAlpha' ? 'selected' : ''}>Src Alpha</option>
                    <option value="add" ${track.blendMode === 'add' ? 'selected' : ''}>Add</option>
                    <option value="multiply" ${track.blendMode === 'multiply' ? 'selected' : ''}>Multiply</option>
                    <option value="screen" ${track.blendMode === 'screen' ? 'selected' : ''}>Screen</option>
                    <option value="overlay" ${track.blendMode === 'overlay' ? 'selected' : ''}>Overlay</option>
                    <option value="difference" ${track.blendMode === 'difference' ? 'selected' : ''}>Difference</option>
                  </select>`
                : `<label class="track-slider-lbl pan-lbl">
                    <span>Pan</span>
                    <input type="range" min="-1" max="1" step="0.05" value="${track.pan}" data-track-pan="${track.id}" />
                  </label>`
            }
          </div>
          <div class="track-resize-grip" data-track-resize="${track.id}" title="Arraste a borda inferior para aumentar ou diminuir a altura desta faixa (${th}px)"></div>
        </div>
      `;
      })
      .join('');

    container.querySelectorAll('.neoveg-track-header').forEach((row) => {
      row.addEventListener('click', (e) => {
        const target = e.target               ;
        if (target && (target.tagName === 'INPUT' || target.tagName === 'SELECT' || target.closest('button'))) {
          return;
        }
        this.timeline.selectedTrackId = (row               ).dataset.trackId || null;
        this.renderTrackHeaders();
      });
    });

    container.querySelectorAll('[data-track-name]').forEach((inp) => {
      const inputEl = inp                    ;
      inputEl.addEventListener('click', (e) => e.stopPropagation());
      inputEl.addEventListener('mousedown', (e) => e.stopPropagation());
      inputEl.addEventListener('dblclick', (e) => {
        e.stopPropagation();
        inputEl.select();
      });
      inputEl.addEventListener('keydown', (e) => {
        e.stopPropagation();
        if (e.key === 'Enter') {
          inputEl.blur();
        }
      });
      inputEl.addEventListener('change', (e) => {
        e.stopPropagation();
        const tid = inputEl.dataset.trackName;
        if (tid) {
          this.renameTrackById(tid, inputEl.value);
        }
      });
    });

    container.querySelectorAll('[data-track-rename]').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const tid = (btn               ).dataset.trackRename ;
        const inp = container.querySelector(`.track-name-input[data-track-name="${tid}"]`)                           ;
        if (inp) {
          inp.focus();
          inp.select();
          this.setStatusMessage(`Digite o novo nome para a faixa "${inp.value}" e pressione Enter.`);
        }
      });
    });

    container.querySelectorAll('[data-track-group]').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const tid = (btn               ).dataset.trackGroup ;
        const tr = this.project.tracks.find((t) => t.id === tid);
        if (!tr) return;
        if (tr.groupId) {
          this.ungroupTracksFlow(tid);
        } else {
          // Pair this track with a companion track of opposite type (or adjacent track)
          const companion =
            this.project.tracks.find((t) => t.id !== tid && t.type !== tr.type && !t.groupId) ||
            this.project.tracks.find((t) => t.id !== tid);
          const ids = companion ? [tid, companion.id] : [tid];
          this.groupAVTracksFlow(ids);
        }
      });
    });

    container.querySelectorAll('[data-track-h-dec]').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const tid = (btn               ).dataset.trackHDec ;
        this.timeline.adjustSingleTrackHeight(tid, -14);
        this.renderTrackHeaders();
      });
    });

    container.querySelectorAll('[data-track-h-inc]').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const tid = (btn               ).dataset.trackHInc ;
        this.timeline.adjustSingleTrackHeight(tid, 14);
        this.renderTrackHeaders();
      });
    });

    container.querySelectorAll('[data-track-resize]').forEach((grip) => {
      grip.addEventListener('mousedown', (e) => {
        e.stopPropagation();
        e.preventDefault();
        const tid = (grip               ).dataset.trackResize ;
        const tr = this.project.tracks.find((t) => t.id === tid);
        if (!tr) return;
        const startY = (e              ).clientY;
        const startH = this.timeline.getTrackHeight(tr);
        const onMove = (ev            ) => {
          const dy = ev.clientY - startY;
          tr.height = Math.max(46, Math.min(220, Math.round(startH + dy)));
          const hdrEl = container.querySelector(`.neoveg-track-header[data-track-id="${tid}"]`)                      ;
          if (hdrEl) hdrEl.style.height = `${tr.height}px`;
          this.timeline.render();
        };
        const onUp = () => {
          window.removeEventListener('mousemove', onMove);
          window.removeEventListener('mouseup', onUp);
          this.renderTrackHeaders();
        };
        window.addEventListener('mousemove', onMove);
        window.addEventListener('mouseup', onUp);
      });
    });

    container.querySelectorAll('[data-track-mute]').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const tid = (btn               ).dataset.trackMute;
        const tr = this.project.tracks.find((t) => t.id === tid);
        if (tr) {
          const nextMuted = !tr.muted;
          tr.muted = nextMuted;
          if (tr.groupId) {
            for (const member of this.project.tracks) {
              if (member.groupId === tr.groupId) member.muted = nextMuted;
            }
            fetch('/api/tracks/group', {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify({
                action: 'sync',
                project: this.project,
                groupId: tr.groupId,
                muted: nextMuted,
              }),
            }).catch(() => {});
          }
          this.renderTrackHeaders();
          this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
        }
      });
    });

    container.querySelectorAll('[data-track-solo]').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const tid = (btn               ).dataset.trackSolo;
        const tr = this.project.tracks.find((t) => t.id === tid);
        if (tr) {
          const nextSolo = !tr.solo;
          tr.solo = nextSolo;
          if (tr.groupId) {
            for (const member of this.project.tracks) {
              if (member.groupId === tr.groupId) member.solo = nextSolo;
            }
            fetch('/api/tracks/group', {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify({
                action: 'sync',
                project: this.project,
                groupId: tr.groupId,
                solo: nextSolo,
              }),
            }).catch(() => {});
          }
          this.renderTrackHeaders();
          this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
        }
      });
    });

    container.querySelectorAll('[data-track-env]').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const tid = (btn               ).dataset.trackEnv;
        const tr = this.project.tracks.find((t) => t.id === tid);
        if (tr) {
          tr.showEnvelope = !tr.showEnvelope;
          this.renderTrackHeaders();
          this.timeline.render();
        }
      });
    });

    container.querySelectorAll('[data-track-level]').forEach((rng) => {
      rng.addEventListener('input', (e) => {
        e.stopPropagation();
        const tid = (rng               ).dataset.trackLevel;
        const tr = this.project.tracks.find((t) => t.id === tid);
        if (tr) {
          tr.level = Number((rng                    ).value);
          const span = rng.parentElement?.querySelector('.track-val');
          if (span) span.textContent = `${Math.round(tr.level * 100)}%`;
          this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
        }
      });
    });

    container.querySelectorAll('[data-track-blend]').forEach((sel) => {
      sel.addEventListener('change', (e) => {
        e.stopPropagation();
        const tid = (sel               ).dataset.trackBlend;
        const tr = this.project.tracks.find((t) => t.id === tid);
        if (tr) {
          tr.blendMode = (sel                     ).value                 ;
          this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
        }
      });
    });

    container.querySelectorAll('[data-track-pan]').forEach((rng) => {
      rng.addEventListener('input', (e) => {
        e.stopPropagation();
        const tid = (rng               ).dataset.trackPan;
        const tr = this.project.tracks.find((t) => t.id === tid);
        if (tr) tr.pan = Number((rng                    ).value);
      });
    });
  }

          renderSettingsTab(container             )       {
    const curRes = `${this.project.width}x${this.project.height}`;
    container.innerHTML = `
      <div class="settings-tab-grid">
        <div class="insp-header">
          <strong>Configurações Avançadas do Projeto & Preferências do Estúdio (Qt / Neoveg 21)</strong>
          <button class="qt-btn qt-btn-primary" id="btn-settings-apply-all">Aplicar e Salvar Configurações</button>
        </div>

        <!-- 1. VIDEO, COLOR SCIENCE & ACEScct 32-BIT FLOAT -->
        <div class="settings-group-box">
          <div class="settings-group-title">1. Vídeo, Ciência de Cor & Pipeline ACEScct 32-Bit Floating Point</div>
          <div class="insp-row-2col">
            <div>
              <label>Nome do Projeto (.veg)</label>
              <input type="text" id="cfg-proj-name" class="neoveg-input" value="${this.project.name}" />
            </div>
            <div>
              <label>Resolução Master</label>
              <select id="cfg-res" class="neoveg-select">
                <option value="1920x1080" ${curRes === '1920x1080' ? 'selected' : ''}>1920 x 1080 (Full HD 1080p)</option>
                <option value="3840x2160" ${curRes === '3840x2160' ? 'selected' : ''}>3840 x 2160 (4K UHD 2160p)</option>
                <option value="2560x1440" ${curRes === '2560x1440' ? 'selected' : ''}>2560 x 1440 (2K QHD)</option>
                <option value="1280x720" ${curRes === '1280x720' ? 'selected' : ''}>1280 x 720 (HD 720p)</option>
                <option value="1080x1920" ${curRes === '1080x1920' ? 'selected' : ''}>1080 x 1920 (Vertical 9:16)</option>
              </select>
            </div>
          </div>

          <div class="insp-row-2col">
            <div>
              <label>Frame Rate (FPS)</label>
              <select id="cfg-fps" class="neoveg-select">
                <option value="23.976" ${this.project.fps === 23.976 ? 'selected' : ''}>23.976 fps (IVTC Film)</option>
                <option value="24" ${this.project.fps === 24 ? 'selected' : ''}>24.000 fps (DCI Cinema)</option>
                <option value="25" ${this.project.fps === 25 ? 'selected' : ''}>25.000 fps (PAL Broadcast)</option>
                <option value="29.97" ${this.project.fps === 29.97 ? 'selected' : ''}>29.970 fps (NTSC SMPTE)</option>
                <option value="30" ${this.project.fps === 30 ? 'selected' : ''}>30.000 fps (Web HD)</option>
                <option value="59.94" ${this.project.fps === 59.94 ? 'selected' : ''}>59.940 fps (Double NTSC)</option>
                <option value="60" ${this.project.fps === 60 ? 'selected' : ''}>60.000 fps (High Frame Rate)</option>
              </select>
            </div>
            <div>
              <label>Pixel Format & Bit Depth</label>
              <select id="cfg-pixfmt" class="neoveg-select">
                <option value="8bit_full" ${this.project.pixelFormat.includes('8bit') ? 'selected' : ''}>8-bit Unsigned Integer (Full Range)</option>
                <option value="32bit_float_video" ${this.project.pixelFormat === '32bit_float_video' ? 'selected' : ''}>32-bit Floating Point (Video Levels)</option>
                <option value="32bit_float_aces" ${this.project.pixelFormat === '32bit_float_aces' ? 'selected' : ''}>32-bit Floating Point (Full Linear ACES 1.3)</option>
              </select>
            </div>
          </div>

          <div class="insp-row-2col">
            <div>
              <label>View Transform / Color Space</label>
              <select id="cfg-colorspace" class="neoveg-select">
                <option value="Rec.709" ${this.project.colorSpace === 'Rec.709' ? 'selected' : ''}>Rec.709 Standard sRGB</option>
                <option value="ACEScct" ${this.project.colorSpace === 'ACEScct' ? 'selected' : ''}>ACEScct Academy Color Encoding</option>
                <option value="Rec.2020_HDR10" ${this.project.colorSpace === 'Rec.2020_HDR10' ? 'selected' : ''}>Rec.2020 ST.2084 PQ (HDR10)</option>
                <option value="DaVinci_WG" ${this.project.colorSpace === 'DaVinci_WG' ? 'selected' : ''}>DaVinci Wide Gamut / Intermediate</option>
              </select>
            </div>
            <div>
              <label>Motion Blur & Resample Mode</label>
              <select id="cfg-resample" class="neoveg-select">
                <option value="disable" ${this.project.resampleMode === 'disable' ? 'selected' : ''}>Disable Resample (Sem Ghosting)</option>
                <option value="optical_flow" ${this.project.resampleMode === 'optical_flow' ? 'selected' : ''}>Optical Flow Warp Interpolation</option>
                <option value="smart_resample" ${this.project.resampleMode === 'smart_resample' ? 'selected' : ''}>Smart Resample (Neoveg Classic)</option>
              </select>
            </div>
          </div>
        </div>

        <!-- 2. AUDIO ENGINE, ASIO / WASAPI & MASTER BUS -->
        <div class="settings-group-box">
          <div class="settings-group-title">2. Motor de Áudio, Driver ASIO/WASAPI, LADSPA & Fairlight Loudness</div>
          <div class="insp-row-2col">
            <div>
              <label>Sample Rate (Hz)</label>
              <select id="cfg-aud-rate" class="neoveg-select">
                <option value="44100" ${this.project.sampleRate === 44100 ? 'selected' : ''}>44,100 Hz (CD Audio)</option>
                <option value="48000" ${this.project.sampleRate === 48000 ? 'selected' : ''}>48,000 Hz (Broadcast DVD/TV)</option>
                <option value="96000" ${this.project.sampleRate === 96000 ? 'selected' : ''}>96,000 Hz (Studio Hi-Res)</option>
                <option value="192000" ${this.project.sampleRate === 192000 ? 'selected' : ''}>192,000 Hz (Ultra Mastering)</option>
              </select>
            </div>
            <div>
              <label>Bit Depth & Master Bus</label>
              <select id="cfg-aud-bus" class="neoveg-select">
                <option value="stereo" ${this.project.masterBusMode === 'stereo' ? 'selected' : ''}>Stereo 2.0 Master Bus (${this.project.audioBitDepth}-bit)</option>
                <option value="surround_51" ${this.project.masterBusMode === 'surround_51' ? 'selected' : ''}>5.1 Surround Cinema (${this.project.audioBitDepth}-bit)</option>
                <option value="dolby_71" ${this.project.masterBusMode === 'dolby_71' ? 'selected' : ''}>7.1 Dolby Atmos Bed (32-bit Float)</option>
              </select>
            </div>
          </div>
          <div class="insp-row-2col">
            <div>
              <label>Audio Device Driver (Windows)</label>
              <select id="cfg-aud-driver" class="neoveg-select">
                <option value="wasapi_shared">Windows WASAPI Low-Latency (Shared/Exclusive)</option>
                <option value="asio_studio">ASIO 2.3 Studio Hardware Driver (64 samples)</option>
                <option value="directsound">Microsoft DirectSound Mapper</option>
              </select>
            </div>
            <div>
              <label>Alvo de Normalização LUFS</label>
              <select id="cfg-aud-lufs" class="neoveg-select">
                <option value="-14">-14.0 LUFS (YouTube / Spotify / Streaming)</option>
                <option value="-23">-23.0 LUFS (EBU R128 Broadcast TV)</option>
                <option value="-24">-24.0 LUFS (ATSC A/85 Cinema/US TV)</option>
              </select>
            </div>
          </div>
        </div>

        <!-- 3. MULTI-MONITOR WORKSPACE & HARDWARE / CORRIDORKEY AI -->
        <div class="settings-group-box">
          <div class="settings-group-title">3. Multi-Monitor (Arrastar para Outros Monitores) & GPU / CorridorKey AI</div>
          <div class="settings-multimonitor-bar">
            <button class="qt-btn qt-btn-primary" data-popout-panel="pane-preview">${Neoveg_ICONS.popoutMonitor}<span>Destacar Preview p/ Outro Monitor</span></button>
            <button class="qt-btn" data-popout-panel="pane-media">${Neoveg_ICONS.popoutMonitor}<span>Destacar Media/FX p/ Monitor 2</span></button>
            <button class="qt-btn" data-popout-panel="pane-mixer">${Neoveg_ICONS.popoutMonitor}<span>Destacar Mixer p/ Monitor 2</span></button>
            <button class="qt-btn" data-popout-panel="pane-timeline">${Neoveg_ICONS.popoutMonitor}<span>Destacar Timeline p/ Monitor 2</span></button>
          </div>
          <div class="insp-row-2col" style="margin-top:6px">
            <div>
              <label>Aceleração de Hardware & Decoder</label>
              <select id="cfg-gpu-mode" class="neoveg-select">
                <option value="safe_nvenc">Hardware NVENC/QSV + Safe Mode (so4compoundplug Fix Ativo)</option>
                <option value="rust_simd">Rust Core SIMD + FFmpeg Software Pipeline</option>
                <option value="corridorkey_cuda">CorridorKey ViT-L Greenscreen Neural Engine (FP16)</option>
              </select>
            </div>
            <div>
              <label>Altura Global das Faixas (Áudio/Vídeo)</label>
              <div style="display:flex;gap:4px">
                <button class="qt-btn" id="btn-cfg-track-shrink" style="flex:1">${Neoveg_ICONS.minus}<span>Diminuir Faixas</span></button>
                <button class="qt-btn" id="btn-cfg-track-expand" style="flex:1">${Neoveg_ICONS.plus}<span>Aumentar Faixas</span></button>
              </div>
            </div>
          </div>
        </div>
      </div>
    `;

    container.querySelectorAll('[data-popout-panel]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const pid = (btn               ).dataset.popoutPanel ;
        this.dockManager.popoutPanelToMonitor(pid);
        this.setStatusMessage(`Painel ${pid} aberto em janela OS nativa — arraste livremente para outro monitor!`);
      });
    });

    document.getElementById('btn-cfg-track-shrink')?.addEventListener('click', () => {
      this.timeline.adjustAllTrackHeights(-12);
      this.renderTrackHeaders();
      this.setStatusMessage('Altura de todas as faixas de Áudio/Vídeo reduzida.');
    });

    document.getElementById('btn-cfg-track-expand')?.addEventListener('click', () => {
      this.timeline.adjustAllTrackHeights(12);
      this.renderTrackHeaders();
      this.setStatusMessage('Altura de todas as faixas de Áudio/Vídeo aumentada.');
    });

    document.getElementById('btn-settings-apply-all')?.addEventListener('click', () => {
      const nameVal = (document.getElementById('cfg-proj-name')                    )?.value || this.project.name;
      const resVal = (document.getElementById('cfg-res')                     )?.value || '1920x1080';
      const [wStr, hStr] = resVal.split('x');
      this.project.name = nameVal;
      this.project.width = Number(wStr || 1920);
      this.project.height = Number(hStr || 1080);
      this.project.fps = Number((document.getElementById('cfg-fps')                     )?.value || 29.97);
      this.project.pixelFormat = (document.getElementById('cfg-pixfmt')                     )?.value || '8bit_full';
      this.project.colorSpace = (document.getElementById('cfg-colorspace')                     )?.value || 'Rec.709';
      this.project.resampleMode = (document.getElementById('cfg-resample')                     )?.value || 'disable';
      this.project.sampleRate = Number((document.getElementById('cfg-aud-rate')                     )?.value || 48000);
      this.project.masterBusMode = (document.getElementById('cfg-aud-bus')                     )?.value || 'stereo';

      this.updateTimecodeReadouts();
      this.timeline.render();
      this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
      this.broadcastMultiMonitorSync('PROJECT');
      this.saveCurrentProjectFlow();
      this.setStatusMessage(
        `Configurações Avançadas salvas: ${this.project.width}x${this.project.height} @ ${this.project.fps}fps • ${this.project.colorSpace} • ${this.project.sampleRate}Hz`
      );
    });
  }

         renderInspectorPanel()       {
    const container = document.getElementById('center-dock-body');
    if (!container) return;

    if (this.activeCenterTab === 'settings') {
      this.renderSettingsTab(container);
      return;
    }

    const clip = this.getSelectedClip();
    if (!clip) {
      container.innerHTML = `
        <div class="empty-note">
          <div>Projeto ativo: <strong>${this.project.name}</strong> (${this.project.width}x${this.project.height} @ ${this.project.fps}fps)</div>
          <div style="margin-top:6px">Arraste qualquer mídia, gerador ou efeito diretamente para a Timeline abaixo, ou escolha uma opção:</div>
          <div style="margin-top:10px;display:flex;gap:6px;justify-content:center;flex-wrap:wrap">
            <button class="qt-btn qt-btn-primary" id="btn-empty-load-demo">Carregar Projeto de Demonstração (.veg)</button>
            <button class="qt-btn" id="btn-empty-open-settings">Aba de Configurações Avançadas</button>
            <button class="qt-btn" id="btn-empty-open-welcome">Janela de Boas-Vindas</button>
          </div>
        </div>
      `;
      document.getElementById('btn-empty-load-demo')?.addEventListener('click', () => {
        this.loadSavedProjectByFile('Neoveg_Broadcast_Demo.veg');
      });
      document.getElementById('btn-empty-open-settings')?.addEventListener('click', () => {
        this.openSettingsTab();
      });
      document.getElementById('btn-empty-open-welcome')?.addEventListener('click', () => {
        this.openWelcomeModal();
      });
      return;
    }

    if (this.activeCenterTab === 'pancrop') {
      const pc = clip.panCrop;
      if (!pc.keyframes || pc.keyframes.length === 0) {
        pc.keyframes = [{ time: 0, x: pc.x, y: pc.y, scale: pc.scale, rotation: pc.rotation }];
      }

      container.innerHTML = `
        <div class="inspector-grid">
          <div class="insp-header">
            <strong>Neoveg Event Pan/Crop: ${clip.name}</strong>
            <span class="insp-sub">Pos: ${(pc.x * 100).toFixed(0)}, ${(pc.y * 100).toFixed(0)} | Zoom: ${Math.round((1 / Math.max(0.2, pc.scale)) * 100)}%</span>
          </div>

          <!-- INTERACTIVE Neoveg PAN/CROP WORKSPACE CANVAS -->
          <div class="pancrop-canvas-wrap">
            <canvas id="pancrop-interactive-canvas" width="280" height="145" title="Arraste o quadro 'F' para mover, o canto para escalar ou o anel externo para girar"></canvas>
            <div class="pancrop-side-tools">
              <button class="qt-btn" id="btn-pc-reset" title="Restaurar Quadro 100%">Reset 100%</button>
              <button class="qt-btn" id="btn-pc-169" title="Predefinição 16:9 Widescreen">16:9 Wide</button>
              <button class="qt-btn" id="btn-pc-pip-tr" title="Picture-in-Picture Canto Superior">PiP Sup-Dir</button>
              <button class="qt-btn" id="btn-pc-pip-bl" title="Picture-in-Picture Canto Inferior">PiP Inf-Esq</button>
              <button class="qt-btn ${pc.flipH ? 'active' : ''}" id="btn-pc-fliph">Espelhar H</button>
              <button class="qt-btn ${pc.flipV ? 'active' : ''}" id="btn-pc-flipv">Espelhar V</button>
            </div>
          </div>

          <div class="insp-row-2col">
            <div>
              <label>Center X (${pc.x.toFixed(2)})</label>
              <input type="range" min="-0.8" max="0.8" step="0.02" value="${pc.x}" id="inp-pc-x" />
            </div>
            <div>
              <label>Center Y (${pc.y.toFixed(2)})</label>
              <input type="range" min="-0.8" max="0.8" step="0.02" value="${pc.y}" id="inp-pc-y" />
            </div>
          </div>

          <div class="insp-row-2col">
            <div>
              <label>Frame Size / Zoom (${Math.round(pc.scale * 100)}%)</label>
              <input type="range" min="0.25" max="2.5" step="0.05" value="${pc.scale}" id="inp-pc-scale" />
            </div>
            <div>
              <label>Rotation Angle (${pc.rotation}°)</label>
              <input type="range" min="-180" max="180" step="1" value="${pc.rotation}" id="inp-pc-rot" />
            </div>
          </div>

          <!-- Neoveg PAN/CROP KEYFRAME CONTROLLER -->
          <div class="pancrop-kf-bar">
            <div class="kf-hdr">
              <span>Keyframes Pan/Crop (${pc.keyframes.length})</span>
              <div style="display:flex;gap:4px">
                <button class="qt-btn qt-btn-primary" id="btn-pc-add-kf">+ Keyframe no Cursor</button>
                <button class="qt-btn" id="btn-pc-clear-kf">Limpar</button>
              </div>
            </div>
            <div class="kf-track-strip" id="pancrop-kf-strip">
              ${pc.keyframes
                .map((kf, idx) => {
                  const leftPct = Math.max(2, Math.min(96, (kf.time / Math.max(0.2, clip.duration)) * 100));
                  return `<div class="kf-diamond" style="left:${leftPct}%" data-kf-idx="${idx}" title="Keyframe ${idx + 1} @ +${kf.time.toFixed(2)}s">◆</div>`;
                })
                .join('')}
            </div>
          </div>
        </div>
      `;

      this.initInteractivePanCropCanvas(clip);

      const syncActiveKeyframe = () => {
        if (clip.panCrop.keyframes && clip.panCrop.keyframes.length > 0) {
          const localT = Math.max(0, Math.min(clip.duration, this.project.playhead - clip.start));
          const nearest = clip.panCrop.keyframes.find((k) => Math.abs(k.time - localT) < 0.25);
          if (nearest) {
            nearest.x = clip.panCrop.x;
            nearest.y = clip.panCrop.y;
            nearest.scale = clip.panCrop.scale;
            nearest.rotation = clip.panCrop.rotation;
          } else if (clip.panCrop.keyframes.length === 1) {
            clip.panCrop.keyframes[0].x = clip.panCrop.x;
            clip.panCrop.keyframes[0].y = clip.panCrop.y;
            clip.panCrop.keyframes[0].scale = clip.panCrop.scale;
            clip.panCrop.keyframes[0].rotation = clip.panCrop.rotation;
          }
        }
      };

      const bindRange = (id        , fn                       ) => {
        const el = document.getElementById(id)                           ;
        el?.addEventListener('input', () => {
          fn(Number(el.value));
          syncActiveKeyframe();
          this.drawPanCropInteractiveCanvas(clip);
          this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
        });
        el?.addEventListener('change', () => this.renderInspectorPanel());
      };

      bindRange('inp-pc-x', (v) => (clip.panCrop.x = v));
      bindRange('inp-pc-y', (v) => (clip.panCrop.y = v));
      bindRange('inp-pc-scale', (v) => (clip.panCrop.scale = v));
      bindRange('inp-pc-rot', (v) => (clip.panCrop.rotation = v));

      document.getElementById('btn-pc-reset')?.addEventListener('click', () => {
        clip.panCrop = {
          x: 0,
          y: 0,
          scale: 1,
          rotation: 0,
          flipH: false,
          flipV: false,
          keyframes: [{ time: 0, x: 0, y: 0, scale: 1, rotation: 0 }],
        };
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
      });
      document.getElementById('btn-pc-169')?.addEventListener('click', () => {
        clip.panCrop.scale = 0.78;
        clip.panCrop.x = 0;
        clip.panCrop.y = 0;
        syncActiveKeyframe();
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
      });
      document.getElementById('btn-pc-pip-tr')?.addEventListener('click', () => {
        clip.panCrop.x = -0.35;
        clip.panCrop.y = 0.32;
        clip.panCrop.scale = 1.65;
        syncActiveKeyframe();
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
      });
      document.getElementById('btn-pc-pip-bl')?.addEventListener('click', () => {
        clip.panCrop.x = 0.35;
        clip.panCrop.y = -0.32;
        clip.panCrop.scale = 1.65;
        syncActiveKeyframe();
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
      });
      document.getElementById('btn-pc-fliph')?.addEventListener('click', () => {
        clip.panCrop.flipH = !clip.panCrop.flipH;
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
      });
      document.getElementById('btn-pc-flipv')?.addEventListener('click', () => {
        clip.panCrop.flipV = !clip.panCrop.flipV;
        this.renderInspectorPanel();
        this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
      });

      document.getElementById('btn-pc-add-kf')?.addEventListener('click', () => {
        const localT = Number(Math.max(0, Math.min(clip.duration, this.project.playhead - clip.start)).toFixed(2));
        clip.panCrop.keyframes = clip.panCrop.keyframes || [];
        clip.panCrop.keyframes.push({
          time: localT,
          x: clip.panCrop.x,
          y: clip.panCrop.y,
          scale: clip.panCrop.scale,
          rotation: clip.panCrop.rotation,
        });
        clip.panCrop.keyframes.sort((a, b) => a.time - b.time);
        this.renderInspectorPanel();
        this.setStatusMessage(`Keyframe Pan/Crop adicionado em +${localT}s`);
      });

      document.getElementById('btn-pc-clear-kf')?.addEventListener('click', () => {
        clip.panCrop.keyframes = [
          { time: 0, x: clip.panCrop.x, y: clip.panCrop.y, scale: clip.panCrop.scale, rotation: clip.panCrop.rotation },
        ];
        this.renderInspectorPanel();
      });
      return;
    }

    if (this.activeCenterTab === 'resolve') {
      // DaVinci Resolve 3-Way Color Wheels + Node Graph + Premiere Lumetri / Text-Based Cut
      container.innerHTML = `
        <div class="inspector-grid">
          <div class="insp-header">
            <strong>Resolve Color Wheels • Node Graph • Premiere Lumetri</strong>
            <button class="qt-btn qt-btn-primary" id="btn-apply-resolve-grade">Aplicar Grade ao Clipe</button>
          </div>

          <div class="color-wheels-row">
            <div class="wheel-box">
              <span>LIFT (Shadows)</span>
              <canvas class="resolve-wheel-canvas" width="72" height="72" data-wheel="lift"></canvas>
            </div>
            <div class="wheel-box">
              <span>GAMMA (Midtones)</span>
              <canvas class="resolve-wheel-canvas" width="72" height="72" data-wheel="gamma"></canvas>
            </div>
            <div class="wheel-box">
              <span>GAIN (Highlights)</span>
              <canvas class="resolve-wheel-canvas" width="72" height="72" data-wheel="gain"></canvas>
            </div>
          </div>

          <!-- FUSION / RESOLVE NODE GRAPH PREVIEW -->
          <div class="node-graph-box">
            <div class="fg-label">DaVinci Resolve / Fusion Node Pipeline:</div>
            <canvas id="resolve-node-canvas" width="350" height="58"></canvas>
          </div>

          <!-- PREMIERE PRO TEXT-BASED TRANSCRIPT EDITING -->
          <div class="transcript-box">
            <div class="fg-label">Premiere Pro Text-Based Transcript Editing (Clique na frase para posicionar/cortar):</div>
            <div class="transcript-tokens">
              <span class="tr-word" data-seek="0.2">[00:00] "Bem-vindos ao Neoveg</span>
              <span class="tr-word" data-seek="1.5">Pro 21 Studio</span>
              <span class="tr-word" data-seek="2.8">com motor Rust,</span>
              <span class="tr-word" data-seek="4.2">Frei0r, LADSPA e CorridorKey!"</span>
            </div>
          </div>
        </div>
      `;

      this.drawResolveWheelsAndNodes();

      container.querySelectorAll('.tr-word').forEach((w) => {
        w.addEventListener('click', () => {
          const t = Number((w               ).dataset.seek || 0);
          this.project.playhead = clip.start + t;
          this.updateTimecodeReadouts();
          this.timeline.render();
          this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
        });
      });

      document.getElementById('btn-apply-resolve-grade')?.addEventListener('click', () => {
        clip.fx.push({
          id: `fx_resolve_${Date.now()}`,
          type: 'colorGrade',
          name: 'DaVinci Resolve 3-Wheel Cinema Grade',
          enabled: true,
          params: { liftR: -0.06, liftB: 0.09, gainR: 1.15, gainB: 0.92, saturation: 1.24 },
        });
        this.timeline.render();
        this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
        this.setStatusMessage(`DaVinci Resolve Cinema Grade aplicado em ${clip.name}`);
      });
      return;
    }

    if (this.activeCenterTab === 'fxchain') {
      const fxItems = clip.fx || [];
      container.innerHTML = `
        <div class="inspector-grid">
          <div class="insp-header">
            <strong>Event FX Chain: ${clip.name} (${fxItems.length})</strong>
            <div style="display:flex;gap:4px">
              <button class="qt-btn qt-btn-primary" id="btn-fx-quick-ck">+ CorridorKey</button>
              <button class="qt-btn" id="btn-fx-quick-grade">+ Color Grade</button>
            </div>
          </div>
          ${
            fxItems.length === 0
              ? `<div class="empty-note">Nenhum efeito ativo neste evento. Use a aba "Video FX" à esquerda ou clique nos botões acima.</div>`
              : fxItems
                  .map(
                    (fx, idx) => `
                  <div class="fx-chain-card">
                    <div class="fx-chain-hdr">
                      <label style="display:flex;align-items:center;gap:6px">
                        <input type="checkbox" data-fx-toggle="${idx}" ${fx.enabled ? 'checked' : ''} />
                        <strong>${fx.name}</strong>
                      </label>
                      <button class="qt-btn" data-fx-del="${idx}">Remover</button>
                    </div>
                    <div class="fx-params-row">
                      ${Object.entries(fx.params)
                        .map(
                          ([k, v]) => `
                        <label class="fx-param-lbl">
                          <span>${k}: ${Number(v).toFixed(2)}</span>
                          <input type="range" min="-0.5" max="2.5" step="0.04" value="${v}" data-fx-idx="${idx}" data-fx-key="${k}" />
                        </label>`
                        )
                        .join('')}
                    </div>
                  </div>`
                  )
                  .join('')
          }
        </div>
      `;

      document.getElementById('btn-fx-quick-ck')?.addEventListener('click', () => {
        clip.fx.push({
          id: `fx_ck_${Date.now()}`,
          type: 'corridorKey',
          name: 'CorridorKey AI Green Screen Unmixer',
          enabled: true,
          params: { similarity: 0.28, despill: 0.9, blend: 0.08 },
        });
        this.renderInspectorPanel();
        this.timeline.render();
        this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
      });

      document.getElementById('btn-fx-quick-grade')?.addEventListener('click', () => {
        clip.fx.push({
          id: `fx_${Date.now()}`,
          type: 'colorGrade',
          name: 'Neoveg Color Corrector (Teal/Orange)',
          enabled: true,
          params: { liftR: -0.05, liftB: 0.08, gainR: 1.14, gainB: 0.92, saturation: 1.2 },
        });
        this.renderInspectorPanel();
        this.timeline.render();
        this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
      });

      container.querySelectorAll('[data-fx-toggle]').forEach((chk) => {
        chk.addEventListener('change', () => {
          const idx = Number((chk               ).dataset.fxToggle);
          clip.fx[idx].enabled = (chk                    ).checked;
          this.timeline.render();
          this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
        });
      });

      container.querySelectorAll('[data-fx-del]').forEach((btn) => {
        btn.addEventListener('click', () => {
          const idx = Number((btn               ).dataset.fxDel);
          clip.fx.splice(idx, 1);
          this.renderInspectorPanel();
          this.timeline.render();
          this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
        });
      });

      container.querySelectorAll('[data-fx-key]').forEach((rng) => {
        rng.addEventListener('input', () => {
          const idx = Number((rng               ).dataset.fxIdx);
          const key = (rng               ).dataset.fxKey ;
          const val = Number((rng                    ).value);
          clip.fx[idx].params[key] = val;
          const span = rng.parentElement?.querySelector('span');
          if (span) span.textContent = `${key}: ${val.toFixed(2)}`;
          this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
        });
      });
      return;
    }

    // Trimmer & Velocity Inspector
    container.innerHTML = `
      <div class="inspector-grid">
        <div class="insp-header">
          <strong>Neoveg Event Trimmer & Velocity: ${clip.name}</strong>
        </div>
        ${
          clip.generatorType === 'titleText'
            ? `
          <div class="insp-row">
            <label>Texto Principal do Título</label>
            <input type="text" class="neoveg-input" id="inp-clip-title" value="${clip.titleText || ''}" />
          </div>
          <div class="insp-row">
            <label>Subtítulo</label>
            <input type="text" class="neoveg-input" id="inp-clip-sub" value="${clip.subtitleText || ''}" />
          </div>`
            : ''
        }
        <div class="insp-row-2col">
          <div>
            <label>Início na Timeline (s)</label>
            <input type="number" step="0.1" class="neoveg-input" id="inp-trim-start" value="${clip.start.toFixed(2)}" />
          </div>
          <div>
            <label>Duração (s)</label>
            <input type="number" step="0.1" class="neoveg-input" id="inp-trim-dur" value="${clip.duration.toFixed(2)}" />
          </div>
        </div>
        <div class="insp-row-2col">
          <div>
            <label>Velocity / Playback Rate (${Math.round(clip.playbackRate * 100)}%)</label>
            <input type="range" min="0.25" max="3.0" step="0.05" value="${clip.playbackRate}" id="inp-trim-rate" />
          </div>
          <div>
            <label>Ganho / Opacidade (${Math.round(clip.gain * 100)}%)</label>
            <input type="range" min="0" max="1.5" step="0.05" value="${clip.gain}" id="inp-trim-gain" />
          </div>
        </div>
        <div class="insp-row-2col">
          <div>
            <label>Curva Fade-In (${clip.fadeIn.toFixed(2)}s)</label>
            <select class="neoveg-select" id="sel-fadein-curve">
              <option value="smooth" ${clip.fadeInCurve === 'smooth' ? 'selected' : ''}>Smooth (S-Curve)</option>
              <option value="fast" ${clip.fadeInCurve === 'fast' ? 'selected' : ''}>Fast (Logarithmic)</option>
              <option value="slow" ${clip.fadeInCurve === 'slow' ? 'selected' : ''}>Slow (Exponential)</option>
              <option value="linear" ${clip.fadeInCurve === 'linear' ? 'selected' : ''}>Linear</option>
              <option value="sharp" ${clip.fadeInCurve === 'sharp' ? 'selected' : ''}>Sharp</option>
            </select>
          </div>
          <div>
            <label>Curva Fade-Out (${clip.fadeOut.toFixed(2)}s)</label>
            <select class="neoveg-select" id="sel-fadeout-curve">
              <option value="smooth" ${clip.fadeOutCurve === 'smooth' ? 'selected' : ''}>Smooth (S-Curve)</option>
              <option value="fast" ${clip.fadeOutCurve === 'fast' ? 'selected' : ''}>Fast (Logarithmic)</option>
              <option value="slow" ${clip.fadeOutCurve === 'slow' ? 'selected' : ''}>Slow (Exponential)</option>
              <option value="linear" ${clip.fadeOutCurve === 'linear' ? 'selected' : ''}>Linear</option>
              <option value="sharp" ${clip.fadeOutCurve === 'sharp' ? 'selected' : ''}>Sharp</option>
            </select>
          </div>
        </div>
      </div>
    `;

    document.getElementById('inp-clip-title')?.addEventListener('input', (e) => {
      clip.titleText = (e.target                    ).value;
      this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
    });
    document.getElementById('inp-clip-sub')?.addEventListener('input', (e) => {
      clip.subtitleText = (e.target                    ).value;
      this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
    });
    document.getElementById('inp-trim-start')?.addEventListener('change', (e) => {
      clip.start = Math.max(0, Number((e.target                    ).value));
      this.timeline.recalculateAutoCrossfades();
      this.timeline.render();
      this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
    });
    document.getElementById('inp-trim-dur')?.addEventListener('change', (e) => {
      clip.duration = Math.max(0.2, Number((e.target                    ).value));
      this.timeline.recalculateAutoCrossfades();
      this.timeline.render();
      this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
    });
    document.getElementById('inp-trim-rate')?.addEventListener('input', (e) => {
      clip.playbackRate = Number((e.target                    ).value);
      this.renderInspectorPanel();
    });
    document.getElementById('inp-trim-gain')?.addEventListener('input', (e) => {
      clip.gain = Number((e.target                    ).value);
      this.timeline.render();
      this.compositor.renderFrame(this.project, clip, this.showPanCropGizmo);
    });
    document.getElementById('sel-fadein-curve')?.addEventListener('change', (e) => {
      clip.fadeInCurve = (e.target                     ).value                 ;
      this.timeline.render();
    });
    document.getElementById('sel-fadeout-curve')?.addEventListener('change', (e) => {
      clip.fadeOutCurve = (e.target                     ).value                 ;
      this.timeline.render();
    });
  }

          initInteractivePanCropCanvas(clip           )       {
    const canvas = document.getElementById('pancrop-interactive-canvas')                            ;
    if (!canvas) return;
    this.drawPanCropInteractiveCanvas(clip);

    let dragMode                                       = 'none';
    let startX = 0;
    let startY = 0;
    let origX = 0;
    let origY = 0;
    let origScale = 1;
    let origRot = 0;

    canvas.addEventListener('mousedown', (e) => {
      const rect = canvas.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      const w = canvas.width;
      const h = canvas.height;
      const pc = clip.panCrop;
      const cx = w / 2 + pc.x * (w * 0.35);
      const cy = h / 2 + pc.y * (h * 0.35);
      const dist = Math.hypot(x - cx, y - cy);
      const boxRadius = Math.hypot((w * 0.55 * pc.scale) / 2, (h * 0.55 * pc.scale) / 2);

      startX = x;
      startY = y;
      origX = pc.x;
      origY = pc.y;
      origScale = pc.scale;
      origRot = pc.rotation;

      if (Math.abs(dist - boxRadius) < 12) {
        dragMode = 'scale';
      } else if (dist > boxRadius + 10) {
        dragMode = 'rotate';
      } else {
        dragMode = 'move';
      }
    });

    window.addEventListener('mousemove', (e) => {
      if (dragMode === 'none') return;
      const rect = canvas.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      const dx = x - startX;
      const dy = y - startY;

      if (dragMode === 'move') {
        clip.panCrop.x = Number(Math.max(-0.8, Math.min(0.8, origX + dx / 95)).toFixed(2));
        clip.panCrop.y = Number(Math.max(-0.8, Math.min(0.8, origY + dy / 55)).toFixed(2));
      } else if (dragMode === 'scale') {
        clip.panCrop.scale = Number(Math.max(0.25, Math.min(2.4, origScale + (dx + dy) / 140)).toFixed(2));
      } else if (dragMode === 'rotate') {
        clip.panCrop.rotation = Math.round(Math.max(-180, Math.min(180, origRot + dx)));
      }

      if (clip.panCrop.keyframes && clip.panCrop.keyframes.length > 0) {
        clip.panCrop.keyframes[clip.panCrop.keyframes.length - 1] = {
          time: clip.panCrop.keyframes[clip.panCrop.keyframes.length - 1].time,
          x: clip.panCrop.x,
          y: clip.panCrop.y,
          scale: clip.panCrop.scale,
          rotation: clip.panCrop.rotation,
        };
      }

      this.drawPanCropInteractiveCanvas(clip);
      this.compositor.renderFrame(this.project, clip, true);
    });

    window.addEventListener('mouseup', () => {
      if (dragMode !== 'none') {
        dragMode = 'none';
        this.renderInspectorPanel();
      }
    });
  }

          drawPanCropInteractiveCanvas(clip           )       {
    const canvas = document.getElementById('pancrop-interactive-canvas')                            ;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const w = canvas.width;
    const h = canvas.height;
    ctx.fillStyle = '#0d1016';
    ctx.fillRect(0, 0, w, h);

    // Coordinate grid (Neoveg Pan/Crop style)
    ctx.strokeStyle = 'rgba(148, 163, 184, 0.16)';
    ctx.lineWidth = 1;
    for (let gx = 0; gx < w; gx += 20) {
      ctx.beginPath();
      ctx.moveTo(gx, 0);
      ctx.lineTo(gx, h);
      ctx.stroke();
    }
    for (let gy = 0; gy < h; gy += 20) {
      ctx.beginPath();
      ctx.moveTo(0, gy);
      ctx.lineTo(w, gy);
      ctx.stroke();
    }

    // Source frame reference rectangle
    const refW = w * 0.62;
    const refH = h * 0.62;
    ctx.strokeStyle = '#475569';
    ctx.strokeRect((w - refW) / 2, (h - refH) / 2, refW, refH);

    // Neoveg interactive 'F' crop frame
    const pc = clip.panCrop;
    const cx = w / 2 + pc.x * (w * 0.35);
    const cy = h / 2 + pc.y * (h * 0.35);
    const bw = refW * pc.scale;
    const bh = refH * pc.scale;

    ctx.save();
    ctx.translate(cx, cy);
    ctx.rotate((pc.rotation * Math.PI) / 180);
    ctx.scale(pc.flipH ? -1 : 1, pc.flipV ? -1 : 1);

    // Rotation ring
    ctx.strokeStyle = 'rgba(56, 189, 248, 0.32)';
    ctx.beginPath();
    ctx.arc(0, 0, Math.hypot(bw / 2, bh / 2), 0, Math.PI * 2);
    ctx.stroke();

    // Dotted crop rectangle
    ctx.strokeStyle = '#38bdf8';
    ctx.lineWidth = 1.6;
    ctx.setLineDash([4, 3]);
    ctx.strokeRect(-bw / 2, -bh / 2, bw, bh);
    ctx.setLineDash([]);

    // Iconic 'F' letter inside crop frame
    ctx.strokeStyle = 'rgba(56, 189, 248, 0.75)';
    ctx.lineWidth = 2.4;
    ctx.beginPath();
    ctx.moveTo(-10, 14);
    ctx.lineTo(-10, -14);
    ctx.lineTo(10, -14);
    ctx.moveTo(-10, 0);
    ctx.lineTo(6, 0);
    ctx.stroke();

    // 4 Corner handles
    ctx.fillStyle = '#38bdf8';
    const corners = [
      [-bw / 2, -bh / 2],
      [bw / 2, -bh / 2],
      [bw / 2, bh / 2],
      [-bw / 2, bh / 2],
    ];
    for (const [hx, hy] of corners) {
      ctx.fillRect(hx - 3.5, hy - 3.5, 7, 7);
    }

    ctx.restore();
  }

          drawResolveWheelsAndNodes()       {
    document.querySelectorAll('canvas.resolve-wheel-canvas').forEach((canv) => {
      const c = canv                     ;
      const ctx = c.getContext('2d') ;
      const r = c.width / 2 - 4;
      const cx = c.width / 2;
      const cy = c.height / 2;

      for (let angle = 0; angle < 360; angle += 6) {
        const rad = (angle * Math.PI) / 180;
        const nextRad = ((angle + 7) * Math.PI) / 180;
        ctx.beginPath();
        ctx.moveTo(cx, cy);
        ctx.arc(cx, cy, r, rad, nextRad);
        ctx.closePath();
        ctx.fillStyle = `hsla(${angle}, 75%, 45%, 0.65)`;
        ctx.fill();
      }
      ctx.strokeStyle = '#94a3b8';
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.arc(cx, cy, r, 0, Math.PI * 2);
      ctx.stroke();

      // Center crosshair puck
      ctx.fillStyle = '#f8fafc';
      ctx.fillRect(cx - 2, cy - 2, 4, 4);
    });

    const nodeCanv = document.getElementById('resolve-node-canvas')                            ;
    if (nodeCanv) {
      const nctx = nodeCanv.getContext('2d') ;
      nctx.fillStyle = '#0c0f15';
      nctx.fillRect(0, 0, nodeCanv.width, nodeCanv.height);
      const nodes = [
        { x: 12, label: 'MediaIn', col: '#2563eb' },
        { x: 96, label: 'CorridorKey', col: '#16a34a' },
        { x: 186, label: '3-Wheel Grade', col: '#d97706' },
        { x: 278, label: 'MediaOut', col: '#0891b2' },
      ];
      nctx.strokeStyle = '#64748b';
      nctx.lineWidth = 1.5;
      nctx.beginPath();
      nctx.moveTo(30, 29);
      nctx.lineTo(310, 29);
      nctx.stroke();

      for (const nd of nodes) {
        nctx.fillStyle = '#1e2430';
        nctx.fillRect(nd.x, 14, 66, 30);
        nctx.strokeStyle = nd.col;
        nctx.strokeRect(nd.x, 14, 66, 30);
        nctx.fillStyle = '#f1f5f9';
        nctx.font = 'bold 9px sans-serif';
        nctx.fillText(nd.label, nd.x + 5, 32);
      }
    }
  }

          showClipContextMenu(clip           , track       , clientX        , clientY        )       {
    const menu = document.getElementById('clip-context-menu');
    if (!menu) return;
    menu.style.left = `${Math.min(window.innerWidth - 220, clientX)}px`;
    menu.style.top = `${Math.min(window.innerHeight - 260, clientY)}px`;
    menu.classList.remove('hidden');

    menu.innerHTML = `
      <div class="ctx-title">${clip.name}</div>
      <button class="ctx-item" data-ctx="split">Split at Playhead (S)</button>
      <button class="ctx-item" data-ctx="pancrop">Event Pan/Crop...</button>
      <button class="ctx-item" data-ctx="fx">Event Video FX / Frei0r...</button>
      <div class="ctx-sep"></div>
      <div class="ctx-sub">Neoveg Fade Curve Type:</div>
      <button class="ctx-item" data-curve="smooth">Smooth S-Curve ${clip.fadeInCurve === 'smooth' ? '✓' : ''}</button>
      <button class="ctx-item" data-curve="fast">Fast Logarithmic ${clip.fadeInCurve === 'fast' ? '✓' : ''}</button>
      <button class="ctx-item" data-curve="slow">Slow Exponential ${clip.fadeInCurve === 'slow' ? '✓' : ''}</button>
      <button class="ctx-item" data-curve="linear">Linear ${clip.fadeInCurve === 'linear' ? '✓' : ''}</button>
      <button class="ctx-item" data-curve="sharp">Sharp ${clip.fadeInCurve === 'sharp' ? '✓' : ''}</button>
      <div class="ctx-sep"></div>
      <button class="ctx-item" data-ctx="dup">Duplicate Event</button>
      <button class="ctx-item danger" data-ctx="del">Delete Event (Del)</button>
    `;

    menu.querySelectorAll('[data-curve]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const c = (btn               ).dataset.curve                 ;
        clip.fadeInCurve = c;
        clip.fadeOutCurve = c;
        this.timeline.render();
        this.renderInspectorPanel();
      });
    });

    menu.querySelectorAll('[data-ctx]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const act = (btn               ).dataset.ctx;
        if (act === 'split') this.splitAtPlayhead();
        else if (act === 'pancrop') {
          this.activeCenterTab = 'pancrop';
          this.showPanCropGizmo = true;
          this.syncCenterTabButtons();
          this.renderInspectorPanel();
          this.compositor.renderFrame(this.project, clip, true);
        } else if (act === 'fx') {
          this.activeCenterTab = 'fxchain';
          this.syncCenterTabButtons();
          this.renderInspectorPanel();
        } else if (act === 'dup') {
          const clone            = JSON.parse(JSON.stringify(clip));
          clone.id = `clip_${Date.now()}`;
          clone.start = Number((clip.start + clip.duration + 0.1).toFixed(3));
          track.clips.push(clone);
          this.timeline.selectedClipId = clone.id;
          this.timeline.render();
        } else if (act === 'del') {
          this.deleteSelectedClip();
        }
      });
    });
  }

          updateTimecodeReadouts()       {
    const tcMain = document.getElementById('neoveg-big-timecode');
    if (tcMain) {
      tcMain.textContent = formatSMPTE(this.project.playhead, this.project.fps, true);
    }
    const tcSub = document.getElementById('neoveg-sub-timecode');
    if (tcSub) {
      const frame = Math.floor(this.project.playhead * this.project.fps);
      tcSub.textContent = `F:${frame} • ${this.project.width}x${this.project.height} @${this.project.fps}p`;
    }
  }

          updateMasterMeters()       {
    const reading = this.audioEngine.syncAudio(this.project, this.isPlaying);
    const barL = document.getElementById('meter-bar-l');
    const barR = document.getElementById('meter-bar-r');
    const peakLbl = document.getElementById('meter-peak-db');

    const normL = Math.max(6, Math.min(100, ((reading.leftDb + 60) / 66) * 100));
    const normR = Math.max(6, Math.min(100, ((reading.rightDb + 60) / 66) * 100));

    if (barL) barL.style.height = `${normL.toFixed(1)}%`;
    if (barR) barR.style.height = `${normR.toFixed(1)}%`;
    if (peakLbl) {
      const maxPeak = Math.max(reading.leftDb, reading.rightDb);
      peakLbl.textContent = maxPeak <= -59 ? '-∞ dB' : `${maxPeak.toFixed(1)} dB`;
      peakLbl.style.color = reading.clipping ? '#ef4444' : '#38bdf8';
    }
  }

         async openDiagnosticsModal()                {
    const modal = document.getElementById('modal-diagnostics');
    if (!modal) return;
    modal.classList.remove('hidden');

    const reportBox = document.getElementById('diag-fixes-list') || document.getElementById('diag-report-output');
    const errLogBox = document.getElementById('diag-error-log-box') || document.getElementById('diag-error-log-output');
    const btnRun = document.getElementById('btn-run-diag-repair');

    const runCheck = async () => {
      if (reportBox) {
        reportBox.innerHTML = `<div class="diag-fix-card">Analisando problemas recorrentes do Neoveg (VFR, So4, Mídia Offline, Envelopes, Auto-Save)...</div>`;
      }
      try {
        const [fixRes, errRes] = await Promise.all([
          fetch('/api/diagnostics/fix-problems', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ project: this.project }),
          }).then((r) => r.json()),
          fetch('/api/diagnostics/errors').then((r) => r.json()),
        ]);

        if (fixRes.ok) {
          this.project = fixRes.project;
          this.timeline.setProject(this.project);
          if (reportBox) {
            const protections = (fixRes.protections_active || [])            ;
            const fixes = (fixRes.fixes_applied || [])            ;
            reportBox.innerHTML = [
              ...protections.map((p) => `<div class="diag-fix-card"><strong>[PROTEÇÃO ATIVA]</strong> ${p}</div>`),
              ...fixes.map((f) => `<div class="diag-fix-card"><strong>[CORREÇÃO APLICADA]</strong> ${f}</div>`),
            ].join('');
          }
          this.setStatusMessage('Diagnóstico e correção preventiva do Neoveg concluídos com sucesso.');
        }
        if (errRes.ok && errLogBox) {
          errLogBox.textContent = (errRes.lines || []).join('\n');
        }
      } catch (err) {
        if (reportBox) reportBox.innerHTML = `<div class="diag-fix-card">Erro na verificação: ${String(err)}</div>`;
      }
    };

    if (btnRun) {
      btnRun.onclick = () => runCheck();
    }
    await runCheck();
  }

         async openRenderModal()                {
    const modal = document.getElementById('modal-render-as');
    if (!modal) return;
    modal.classList.remove('hidden');

    const fgBox = document.getElementById('render-filtergraph-preview');
    const statusBox = document.getElementById('render-output-status');
    if (fgBox) fgBox.textContent = 'Compilando timeline via Rust Core / Python FFmpeg + Frei0r + LADSPA + CorridorKey...';
    if (statusBox) statusBox.innerHTML = '';

    try {
      const res = await fetch('/api/timeline/compile', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ project: this.project, outputName: 'Neoveg_Master_Output.mp4' }),
      }).then((r) => r.json());

      if (res.ok && fgBox) {
        fgBox.textContent = `# Auto-Crossfades: ${res.crossfades?.length || 0}\n${res.ffmpeg_plan?.command_str || ''}`;
      }
    } catch (err) {
      if (fgBox) fgBox.textContent = `Filtergraph Offline Preview: ${String(err)}`;
    }

    const startBtn = document.getElementById('btn-start-ffmpeg-render');
    if (startBtn) {
      startBtn.onclick = async () => {
        if (statusBox) {
          statusBox.innerHTML = `<div class="render-progress-pill">Renderizando MP4 multi-pista via FFmpeg 9.0.2...</div>`;
        }
        try {
          const outName = (document.getElementById('inp-render-filename')                    )?.value || 'Neoveg_Master.mp4';
          const r = await fetch('/api/render/run', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ project: this.project, outputName: outName }),
          }).then((x) => x.json());

          if (r.ok && statusBox) {
            const kb = Math.round((r.sizeBytes || 0) / 1024);
            statusBox.innerHTML = `
              <div class="render-success-card">
                <div><strong>Renderização Concluída em ${r.elapsedSec}s!</strong> (${kb} KB • ${r.crossfadesCount} crossfades)</div>
                <a class="qt-btn qt-btn-primary" href="${r.url}" download="${r.fileName}" target="_blank">Baixar / Abrir ${r.fileName}</a>
              </div>
            `;
            this.setStatusMessage(`Renderizado ${r.fileName} (${kb} KB) via FFmpeg`);
          } else if (statusBox) {
            statusBox.innerHTML = `<div class="render-error-card">Falha na renderização: ${r.error || 'Erro desconhecido'}</div>`;
          }
        } catch (err) {
          if (statusBox) statusBox.innerHTML = `<div class="render-error-card">${String(err)}</div>`;
        }
      };
    }
  }

         openScriptingModal()       {
    const modal = document.getElementById('modal-scripting');
    if (!modal) return;
    modal.classList.remove('hidden');

    const presetContainer = document.getElementById('script-preset-buttons');
    const codeArea = document.getElementById('script-code-editor')                              ;
    const logBox = document.getElementById('script-output-console');

    if (presetContainer) {
      presetContainer.innerHTML = Object.entries(this.scriptPresets)
        .map(
          ([key, p]) =>
            `<button class="qt-btn" data-script-preset="${key}" title="${p.description}">${p.name}</button>`
        )
        .join('');

      presetContainer.querySelectorAll('[data-script-preset]').forEach((btn) => {
        btn.addEventListener('click', () => {
          const k = (btn               ).dataset.scriptPreset ;
          if (codeArea && this.scriptPresets[k]) {
            codeArea.value = this.scriptPresets[k].code;
          }
        });
      });
    }

    if (codeArea && !codeArea.value.trim()) {
      const firstPreset = Object.values(this.scriptPresets)[0];
      codeArea.value =
        firstPreset?.code ||
        `# Neoveg Python Scripting API\nneoveg.add_marker(neoveg.cursor, "Script Marker", "#38bdf8")\nneoveg.log("Marker added at cursor!")\n`;
    }

    const runBtn = document.getElementById('btn-execute-python-script');
    if (runBtn) {
      runBtn.onclick = async () => {
        if (!codeArea) return;
        if (logBox) logBox.textContent = 'Executando script Python na timeline...';
        try {
          const res = await fetch('/api/scripts/execute', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ code: codeArea.value, project: this.project }),
          }).then((r) => r.json());

          if (res.ok) {
            this.project = res.project;
            this.timeline.setProject(this.project);
            this.renderTrackHeaders();
            this.renderInspectorPanel();
            this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
            if (logBox) {
              logBox.textContent = (res.logs || []).join('\n') || res.stdout || 'Script executado com sucesso.';
            }
          } else if (logBox) {
            logBox.textContent = `Erro: ${res.error}`;
          }
        } catch (err) {
          if (logBox) logBox.textContent = `Erro ao comunicar com backend Python: ${String(err)}`;
        }
      };
    }
  }

          setStatusMessage(msg        )       {
    const el = document.getElementById('neoveg-footer-status');
    if (el) el.textContent = msg;
  }

         exportProjectJson()         {
    return JSON.stringify(this.project, null, 2);
  }

         loadProjectJson(rawJson        )       {
    try {
      const parsed = JSON.parse(rawJson)                ;
      if (parsed && Array.isArray(parsed.tracks)) {
        this.project = parsed;
        this.timeline.setProject(this.project);
        this.renderTrackHeaders();
        this.renderInspectorPanel();
        this.updateTimecodeReadouts();
        this.compositor.renderFrame(this.project, this.getSelectedClip(), this.showPanCropGizmo);
        this.setStatusMessage(`Projeto carregado: "${this.project.name}"`);
      }
    } catch (err) {
      this.logErrorToBackend('LOAD_VEG_JSON', String(err));
    }
  }
}

                
                    
                                      
   
 

window.addEventListener('DOMContentLoaded', () => {
  const app = new NeovegStudioApp();
  window.openNeovegApp = app;
  app.init().catch(console.error);
});


//# sourceURL=src/main.ts
