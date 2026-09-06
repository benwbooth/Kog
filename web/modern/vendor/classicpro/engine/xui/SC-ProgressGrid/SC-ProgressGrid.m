#include <lib/std.mi>

Function refreshPos();

Global Group XUIGroup, myGrid;
Global Layer grid_L, grid_M, grid_R;
Global Slider fakeSlider;
Global int i_left, i_right;

System.onScriptLoaded(){
	XUIGroup = getScriptGroup();
	myGrid = XUIGroup.findObject("sc.progressgrid");
	grid_L = XUIGroup.findObject("sc.pg.left");
	grid_M = XUIGroup.findObject("sc.pg.center");
	grid_R = XUIGroup.findObject("sc.pg.right");
	fakeSlider = XUIGroup.findObject("sc.seeker");
	refreshPos();
	i_left = 10;
	i_right = 10;
}

System.onSetXuiParam(String param, String value) {
	if(strlower(param) == "left"){
		grid_L.setXmlParam("image", value);
		Map m = new Map;
		m.loadMap(value);
		i_left = m.getWidth();
		delete m;
		grid_M.setXmlParam("x", integerToString(i_left));
	}
	else if(strlower(param) == "middle"){
		grid_M.setXmlParam("image", value);
	}
	else if(strlower(param) == "right"){
		grid_R.setXmlParam("image", value);
		Map m = new Map;
		m.loadMap(value);
		i_right = m.getWidth();
		delete m;
		grid_R.setXmlParam("x", integerToString(-i_right));
	}
}


XUIGroup.onSetVisible(boolean onOff){
	if(onOff) refreshPos();
}

fakeSlider.onSetPosition(int newpos){
	//refreshPos();
}

fakeSlider.onPostedPosition(int newpos){
	refreshPos();
}

fakeSlider.onSetFinalPosition(int pos){
	//refreshPos();
}

System.onSeek(int newpos){
	refreshPos();
}

System.onStop(){
	refreshPos();
}

XUIGroup.onResize(int x, int y, int w, int h){
	refreshPos();
}

refreshPos(){
	int w = XUIGroup.getWidth();
	int len = System.getPlayItemLength();
	
	int proW;
	if(len!=0) proW = (w+0)/len*System.getPosition(); //add 2 to the width - so that the progress and thumb doesnt loose contact
	else proW=0;
	
	if(proW>w) proW=w;
	
	if(proW<i_left+i_right){
		grid_M.setXmlParam("w", integerToString(-i_left));
		grid_R.hide();
	}
	else{
		grid_M.setXmlParam("w", integerToString(-i_left-i_right));
		grid_R.show();
	}
	myGrid.setXmlParam("w", integerToString(proW));
}