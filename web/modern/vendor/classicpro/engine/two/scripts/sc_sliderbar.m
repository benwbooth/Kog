#include <lib/std.mi>

Global Group myGroup;
Global Slider mySlider;
Global Layer myLayer1, myLayer2;
Global int max;
Global boolean md;

System.onScriptLoaded() {
	myGroup = getScriptGroup();
	mySlider = myGroup.getObject(getToken(getParam(),";",0));
	myLayer1 = myGroup.getObject(getToken(getParam(),";",1)+"1");
	myLayer2 = myGroup.getObject(getToken(getParam(),";",1)+"2");
	max = stringToInteger(getToken(getParam(),";",2));

	myLayer1.setXmlParam("w", integerToString(max*mySlider.getPosition()/255));
	myLayer2.setXmlParam("w", integerToString(max*mySlider.getPosition()/255));
}

mySlider.onSetPosition(int newPos){
	myLayer1.setXmlParam("w", integerToString(max*newPos/255));
	myLayer2.setXmlParam("w", integerToString(max*newPos/255));
}
mySlider.onPostedPosition(int newPos){
	myLayer1.setXmlParam("w", integerToString(max*newPos/255));
	myLayer2.setXmlParam("w", integerToString(max*newPos/255));
}
mySlider.onEnterArea(){
	myLayer2.setTargetW(max*mySlider.getPosition()/255);
	myLayer2.setTargetA(255);
	myLayer2.setTargetSpeed(0.2);
	myLayer2.gotoTarget();
	
}
mySlider.onLeaveArea(){
	if(md) return;
	myLayer2.cancelTarget();
	myLayer2.setTargetW(max*mySlider.getPosition()/255);
	myLayer2.setTargetA(0);
	myLayer2.setTargetSpeed(0.8);
	myLayer2.gotoTarget();
}

mySlider.onLeftButtonDown(int x, int y){md = true;}
mySlider.onLeftButtonUp(int x, int y){
	md = false;
	if(!mySlider.isMouseOverRect()) mySlider.onLeaveArea();
}