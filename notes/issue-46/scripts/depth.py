import sys, glob, numpy as np
d=[]; n=0; land=0
for f in sorted(glob.glob(sys.argv[1]+'/seed-*/cells_*.csv')):
    t=int(f[-9:-4])
    if t<500: continue
    hdr=open(f).readline().strip().split(',')
    a=np.loadtxt(f,delimiter=',',skiprows=1,usecols=[hdr.index('lake'),hdr.index('ocean')])
    l=a[:,0][(a[:,1]==0)]
    land+=len(l); d.append(l[l>0])
d=np.concatenate(d)
print('lake cells (>0) % of land', 100*len(d)/land, ' >=1 m', 100*(d>=1).sum()/land)
for q in [10,25,50,75,90]: print(q, np.percentile(d,q))
for th in [1,5,10,20,50]: print('<',th,'m: % of lake cells', 100*(d<th).mean())
