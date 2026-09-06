#include <lib/std.mi>

#define i_info 40 


Function saveResize(int x, int y, int w, int h);

Global Group mainGroup;
Global Container main;
Global Layout mylayout;

//Global Layer resize1, resize2, resize3, resize4, resize6, resize7, resize8, resize9;
Global Layer resize6;
Global Boolean mouseDown, docked;
Global int rres, i_titlebar;

System.onScriptLoaded() {
	rres=20;
	mainGroup = getScriptGroup();

	/*Map m = new Map;
	m.loadMap("frame.top");
	i_titlebar = m.getHeight();
	delete m;*/

	
	main = System.getContainer("main");
	mylayout = main.getLayout("shade");
	
	//resize1 = mainGroup.findObject("resizer.1");
	//resize2 = mainGroup.findObject("resizer.2");
	//resize3 = mainGroup.findObject("resizer.3");
	//resize4 = mainGroup.findObject("resizer.4");
	resize6 = mainGroup.getObject("resizer.6");
	//resize7 = mainGroup.findObject("resizer.7");
	//resize8 = mainGroup.getObject("resizer.8");
	//resize9 = mainGroup.getObject("resizer.9");

}
System.onScriptUnloading(){

}



/*	Classic Resizers ;)
*/
resize6.onLeftButtonDown(int x, int y){mouseDown=true;}
resize6.onLeftButtonUp(int x, int y){mouseDown=false;}
resize6.onMouseMove(int x, int y){
	if(mouseDown){
		x=x-(x-240)%rres;
		saveResize(myLayout.getLeft(), myLayout.getTop(),x+10, 22);
	}
}
/*
resize8.onLeftButtonDown(int x, int y){mouseDown=true;}
resize8.onLeftButtonUp(int x, int y){mouseDown=false;}
resize8.onMouseMove(int x, int y){
	if(mouseDown){
		y=y-(y-168)%rres;
		saveResize(myLayout.getLeft(), myLayout.getTop(), myLayout.getWidth(),y+10);
	}
}
resize9.onLeftButtonDown(int x, int y){mouseDown=true;}
resize9.onLeftButtonUp(int x, int y){mouseDown=false;}
resize9.onMouseMove(int x, int y){
	if(mouseDown){
		x=x-(x-240)%rres;
		y=y-(y-168)%rres;
		saveResize(myLayout.getLeft(), myLayout.getTop(),x+10,y+10);
	}
}*/


saveResize(int x, int y, int w, int h){
	if(docked) return;
	//if(getPublicInt("cPro2.fs", 0)==1) return;
	
	if(w<240) w=240;
	//if(h<i_titlebar+i_info+30+8+40) h=i_titlebar+i_info+30+8;
	
	myLayout.resize(x,y,w,h);
}


mylayout.onDock(int side){
	docked=true;
}

mylayout.onUndock(){
	docked=false;
}