import sys, glob, numpy as np
A=4*np.pi*6371.0**2/40962  # km²
rows=[]
for f in sorted(glob.glob(sys.argv[1]+'/seed-*/cells_*.csv')):
    t=int(f[-9:-4])
    if t<500: continue
    hdr=open(f).readline().strip().split(',')
    a=np.loadtxt(f,delimiter=',',skiprows=1,usecols=[hdr.index(c) for c in ('lake','ocean','elev','lake_evap','lat')])
    lk=(a[:,1]==0)&(a[:,0]>0); e=np.nan_to_num(a[:,3])
    below=lk&(a[:,2]<0); trop=lk&(np.abs(a[:,4])<30)
    rows.append((e[lk].sum()*A/1e3, e[below].sum()*A/1e3, e[lk&~below].sum()*A/1e3, e[trop].sum()*A/1e3, np.mean(e[lk]) if lk.any() else 0))
r=np.array(rows)
print('mean km3/yr: all %.0f below-sea floors %.0f above %.0f tropical(<30deg) %.0f ; mean excess m/yr %.2f'%tuple(r.mean(0)))
print('median', np.median(r,0))
