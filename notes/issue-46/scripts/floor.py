import sys, glob, numpy as np
rows=[]
for f in sorted(glob.glob(sys.argv[1]+'/seed-*/cells_*.csv')):
    t=int(f[-9:-4])
    if t<500: continue
    hdr=open(f).readline().strip().split(',')
    a=np.loadtxt(f,delimiter=',',skiprows=1,usecols=[hdr.index(c) for c in ('lake','ocean','elev')])
    land=a[:,1]==0; lk=land&(a[:,0]>0); L=land.sum()
    below=lk&(a[:,2]<0)
    rows.append((f.split('/')[-2],100*lk.sum()/L,100*below.sum()/L,100*(lk&~below).sum()/L,100*(lk&~below&(a[:,0]<200)).sum()/L))
r=np.array([x[1:] for x in rows])
print('mean: all lakes %.2f  floor<0 %.2f  floor>=0 %.2f  floor>=0 & depth<200m %.2f'%tuple(r.mean(0)))
print('median snapshot:', np.median(r,0))
